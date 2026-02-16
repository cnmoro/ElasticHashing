use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// A single entry in the hash table.
struct Entry {
    key: u64,
    value: PyObject,
}

/// Represents one of the A_i arrays described in the paper.
struct SubArray {
    slots: Vec<Option<Entry>>,
    count: usize,
    capacity: usize,
}

/// Simple GCD helper to ensure probe sequence covers the whole array
fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

impl SubArray {
    fn new(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(None);
        }
        SubArray {
            slots,
            count: 0,
            capacity,
        }
    }

    fn load_factor(&self) -> f64 {
        if self.capacity == 0 { return 1.0; }
        self.count as f64 / self.capacity as f64
    }

    fn epsilon(&self) -> f64 {
        1.0 - self.load_factor()
    }

    /// Helper to generate Double Hashing parameters (h1, h2)
    /// Ensures h2 is coprime to capacity so we visit all slots.
    fn hash_key(&self, key: u64) -> (usize, usize) {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        self.capacity.hash(&mut hasher); 
        let full_hash = hasher.finish();
        
        let h1 = full_hash as usize;
        
        // Initial guess for step size (odd number)
        let mut h2 = ((full_hash >> 32) as usize) | 1;

        // CRITICAL FIX: Ensure gcd(h2, capacity) == 1
        // If they share a factor, the probe sequence will cycle early 
        // and we won't find empty slots.
        while gcd(h2, self.capacity) != 1 {
            h2 = h2.wrapping_add(2); // Keep it odd, try next
            // If h2 wraps around to 1, we are fine (linear probing)
            if h2 == 1 { break; } 
        }
        
        (h1, h2)
    }

    fn insert_probe(&mut self, key: u64, val: PyObject, limit: usize, force: bool) -> (bool, usize) {
        if self.capacity == 0 { return (false, 0); }

        let (h1, h2) = self.hash_key(key);
        let loop_limit = if force { self.capacity } else { limit };

        for i in 0..loop_limit {
            // Safe Double Hashing
            let idx = (h1.wrapping_add(i.wrapping_mul(h2))) % self.capacity;
            
            match &self.slots[idx] {
                None => {
                    self.slots[idx] = Some(Entry { key, value: val });
                    self.count += 1;
                    return (true, i + 1);
                }
                Some(entry) => {
                    if entry.key == key {
                        self.slots[idx] = Some(Entry { key, value: val });
                        return (true, i + 1);
                    }
                }
            }
        }
        (false, loop_limit)
    }

    fn get(&self, py: Python<'_>, key: u64) -> Option<PyObject> {
        if self.capacity == 0 { return None; }

        let (h1, h2) = self.hash_key(key);
        
        for i in 0..self.capacity {
            let idx = (h1.wrapping_add(i.wrapping_mul(h2))) % self.capacity;
            
            match &self.slots[idx] {
                Some(entry) => {
                    if entry.key == key {
                        return Some(entry.value.clone_ref(py));
                    }
                },
                None => return None, 
            }
        }
        None
    }
}

#[pyclass]
struct ElasticTable {
    subarrays: Vec<SubArray>,
    #[allow(dead_code)]
    total_capacity: usize,
    delta: f64,
    c_param: f64,
}

#[pymethods]
impl ElasticTable {
    /// Create a new ElasticTable with specified capacity and delta parameter.
    /// 
    /// Args:
    ///     capacity: Total number of slots in the hash table
    ///     delta: Elasticity parameter (default: 0.05). Target load factor = 1 - delta.
    ///            Lower delta = higher load factor but may increase probe count.
    ///            Recommended range: 0.05 to 0.20
    #[new]
    #[pyo3(signature = (capacity, delta=0.05))]
    fn new(capacity: usize, delta: f64) -> PyResult<Self> {
        if delta <= 0.0 || delta >= 1.0 {
            return Err(PyValueError::new_err("delta must be between 0 and 1"));
        }
        
        let mut subarrays = Vec::new();
        let mut remaining = capacity;
        
        while remaining > 0 {
            let size = if remaining < 16 { 
                remaining 
            } else { 
                (remaining as f64 / 2.0).ceil() as usize 
            };
            
            subarrays.push(SubArray::new(size));
            remaining = remaining.saturating_sub(size);
        }

        Ok(ElasticTable {
            subarrays,
            total_capacity: capacity,
            delta,
            c_param: 2.0, 
        })
    }

    /// Create an ElasticTable sized for a specific number of expected items.
    /// 
    /// Args:
    ///     expected_items: The number of items you plan to store
    ///     load_factor: Target load factor (default: 0.90). Must be between 0.5 and 0.98.
    ///                  Higher values = more space-efficient but slightly more probes.
    ///                  Recommended: 0.85-0.95
    /// 
    /// Example:
    ///     table = ElasticTable.for_items(1000000)  # Stores ~1M items at 90% load
    ///     table = ElasticTable.for_items(1000000, 0.95)  # More space-efficient
    #[staticmethod]
    #[pyo3(signature = (expected_items, load_factor=0.90))]
    fn for_items(expected_items: usize, load_factor: f64) -> PyResult<Self> {
        if load_factor <= 0.5 || load_factor >= 0.99 {
            return Err(PyValueError::new_err(
                "load_factor must be between 0.5 and 0.99"
            ));
        }
        
        // Calculate capacity needed for the expected items at the target load factor
        let capacity = ((expected_items as f64) / load_factor).ceil() as usize;
        
        // Delta is the "empty space" parameter: 1 - load_factor
        let delta = 1.0 - load_factor;
        
        Self::new(capacity, delta)
    }

    fn insert(&mut self, py: Python<'_>, key: u64, value: PyObject) -> PyResult<usize> {
        let n_arrays = self.subarrays.len();
        let mut total_probes = 0;

        for i in 0..n_arrays {
            let has_next = i < n_arrays - 1;
            
            let eps1 = self.subarrays[i].epsilon();
            let eps2 = if has_next { self.subarrays[i+1].epsilon() } else { 0.0 };

            let safe_eps = if eps1 < 1e-9 { 1e-9 } else { eps1 };
            let log_term = (1.0 / safe_eps).log2();
            let limit = (self.c_param * log_term.powi(2)).ceil() as usize;

            let is_case_1 = eps1 > (self.delta / 2.0) && eps2 > 0.25;
            let is_case_2 = eps1 <= (self.delta / 2.0);
            let is_case_3 = eps2 <= 0.25; 

            let attempt = |sub: &mut SubArray, limit: usize, force: bool| -> (bool, usize) {
                let (success, p) = sub.insert_probe(key, value.clone_ref(py), limit, force);
                (success, p)
            };

            let (success, probes) = if is_case_1 {
                attempt(&mut self.subarrays[i], limit, false)
            } else if is_case_2 {
                (false, 0)
            } else if is_case_3 || !has_next {
                let (s, p) = attempt(&mut self.subarrays[i], 0, true);
                if !s && !has_next {
                     return Err(PyValueError::new_err("Hash table is completely full"));
                }
                (s, p)
            } else {
                attempt(&mut self.subarrays[i], limit, false)
            };

            total_probes += probes;

            if success {
                return Ok(total_probes);
            }
        }

        Err(PyValueError::new_err("Could not insert key"))
    }

    fn get(&self, py: Python<'_>, key: u64) -> Option<PyObject> {
        for sub in &self.subarrays {
            if let Some(val) = sub.get(py, key) {
                return Some(val);
            }
        }
        None
    }

    fn stats(&self) -> Vec<(usize, usize, f64)> {
        self.subarrays.iter().enumerate().map(|(i, sub)| {
            (i, sub.count, sub.load_factor())
        }).collect()
    }
}

#[pymodule]
fn elastic_hash(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ElasticTable>()?;
    Ok(())
}