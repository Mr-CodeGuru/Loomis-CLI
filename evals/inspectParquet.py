"""
inspect_parquet.py — one-off diagnostic to inspect dbe/embeddings.parquet's real schema.
NOT part of the sidecar or the shipped project — run once, report findings, then this
script's job is done. Requires pyarrow (not a sidecar runtime dependency, install separately).

Usage (from project root, venv activated):
    python evals/inspectParquet.py
"""

from pathlib import Path
import pyarrow.parquet as pq

root = Path(__file__).parent.parent
parquet_path = root / "dbe" / "embeddings.parquet"
if not parquet_path.exists():
    parquet_path = root / "db" / "embeddings.parquet"

print(f"Reading schema from: {parquet_path}\n")

pf = pq.ParquetFile(parquet_path)
schema = pf.schema_arrow

print("=== Schema ===")
for field in schema:
    print(f"  {field.name}: {field.type}")

print(f"\n=== Row count ===")
print(f"  {pf.metadata.num_rows}")

print(f"\n=== Row groups ===")
print(f"  {pf.metadata.num_row_groups}")

print("\n=== Sample row (first row) ===")
table = pf.read_row_group(0)
first_row = table.slice(0, 1).to_pylist()[0]
for key, value in first_row.items():
    display_value = value
    # Truncate long vector fields for readability
    if isinstance(value, list) and len(value) > 8:
        display_value = f"[{value[0]:.4f}, {value[1]:.4f}, ... ({len(value)} dims total)]"
    print(f"  {key}: {display_value}")
