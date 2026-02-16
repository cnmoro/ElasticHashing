import elastic_hash
import random
import statistics

N = 1_000_000
DELTA = 0.05 

print(f"--- Benchmarking Elastic Hash (Size: {N}, Target Load: 95%) ---")
table = elastic_hash.ElasticTable(N, DELTA)
keys = list(range(int(N * (1 - DELTA))))
random.shuffle(keys)

probe_counts = []

for k in keys:
    # insert now returns the number of probes
    probes = table.insert(k, "val")
    probe_counts.append(probes)

print(f"\n--- Results ---")
print(f"Total Insertions: {len(keys)}")
print(f"Average Probes:   {statistics.mean(probe_counts):.4f}")
print(f"Max Probes:       {max(probe_counts)}")
print(f"Variance:         {statistics.variance(probe_counts):.4f}")

# # Check the distribution of costs
# print("\n--- Cost Distribution ---")
costs = [0]*10
for p in probe_counts:
    if p < 10: costs[p] += 1
    
for i in range(1, 10):
    print(f"{i} probe(s): {costs[i]} items")