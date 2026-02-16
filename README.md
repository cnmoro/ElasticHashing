# Elastic Hashing (Rust + Python)

A high-performance, open-addressing hash table implementation based on the 2025 paper **"Optimal Bounds for Open Addressing Without Reordering"**.

This library is written in **Rust** and exposed to **Python** via PyO3. It achieves **O(1) amortized probe complexity** even at extremely high load factors (95%+), without needing to move elements after insertion (no Robin Hood or Cuckoo hashing required).

## Features

*   **Theoretical Breakthrough:** Implements the *Elastic Hashing* algorithm (Farach-Colton, Krapivin, Kuszmaul, 2025).
*   **High Load Efficiency:** Maintains performance stability up to 95% load factor.
*   **No Reordering:** Keys are never moved once inserted, making it suitable for scenarios where pointer stability is preferred.
*   **Double Hashing:** Uses GCD-guaranteed double hashing to eliminate primary clustering and minimize variance.
*   **Thread Safety:** Fully compatible with Python's GIL.

## Installation

### From Source

```bash
pip install maturin
maturin develop --release
```

### From pypi

```bash
pip install rb-elastic-hash
```

## Usage

```python
import rb_elastic_hash

# Easy way: Specify how many items you want to store
# This will automatically size the table for 90% load factor (default)
table = rb_elastic_hash.ElasticTable.for_items(1_000_000)

# Want higher space efficiency? Increase the load factor
table = rb_elastic_hash.ElasticTable.for_items(1_000_000, load_factor=0.95)

# Advanced: Create with explicit capacity and delta parameter
# Delta 0.05 implies a target load factor of 95%
table = rb_elastic_hash.ElasticTable(1_000_000, delta=0.05)

# Insert items
# Returns the number of probes used for the insertion
probes = table.insert(42, "Meaning of life")
print(f"Inserted in {probes} probes.")

# Retrieve items
value = table.get(42)
print(value)  # Output: "Meaning of life"
```

### API Reference

#### `ElasticTable.for_items(expected_items, load_factor=0.90)`
**Recommended for most users.** Creates a table sized to store `expected_items` at the specified load factor.

- `expected_items`: Number of items you plan to insert
- `load_factor`: Target load factor (default: 0.90). Higher = more space-efficient. Range: 0.5-0.98

#### `ElasticTable(capacity, delta=0.05)`
Advanced constructor. Creates a table with a specific slot capacity.

- `capacity`: Total number of slots (not items)
- `delta`: Elasticity parameter (default: 0.05). Target load factor = 1 - delta. Range: 0.01-0.5

## Benchmarks

In tests with **1,000,000 items** at **95% Load Factor**:

| Metric | Standard Linear Probing | Standard Double Hashing | **Elastic Hashing** |
| :--- | :--- | :--- | :--- |
| **Average Probes** | ~200 | ~20 | **~3.6** |
| **Max Probes** | > 5,000 | ~500 | **~320** |

*Elastic Hashing achieves ~5.5x better probe efficiency than standard Double Hashing and ~50x better than Linear Probing at this load.*

## How It Works

Elastic Hashing divides the table into geometrically decreasing subarrays ($A_0, A_1, \dots$). 

1.  **Greedy Insert:** It tries to insert into array $A_i$.
2.  **Elastic Skip:** If $A_i$ becomes too full (based on a calculated threshold f(ε)), the algorithm "skips" $A_i$ and attempts to insert into the next, smaller array $A_{i+1}$.
3.  **Backpressure:** If deeper arrays fill up, they force previous arrays to accept more elements, balancing the load perfectly across the structure.

This approach overcomes the "Coupon Collector Problem" that typically destroys performance in high-load open-addressing tables.

## Reference

Based on:
> *Optimal Bounds for Open Addressing Without Reordering*
> Martín Farach-Colton, Andrew Krapivin, William Kuszmaul
> arXiv:2501.02305v2 [cs.DS] (2025)
