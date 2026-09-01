"""
loadJina.py — standalone verification that jina-embeddings-v2-base-code loads and
embeds correctly in this venv. Run this BEFORE building the sidecar around it.

Location: LoomisCLI\\loadModels\\loadJina.py
Usage (with venv activated, from project root):
    python loadModels\\loadJina.py
"""

import os
from pathlib import Path

# .parent.parent because this script lives one level deep, in loadModels\ —
# CACHE_DIR must always resolve to the project root's models\, not loadModels\models\
CACHE_DIR = Path(__file__).parent.parent / "models"
os.environ["HF_HOME"] = str(CACHE_DIR)

from sentence_transformers import SentenceTransformer

MODEL_NAME = "jinaai/jina-embeddings-v2-base-code"

print(f"Loading {MODEL_NAME} into {CACHE_DIR} ...")
model = SentenceTransformer(MODEL_NAME, trust_remote_code=True)
print("Model loaded successfully.")

sample_text = "def add(a, b):\n    return a + b"
embedding = model.encode(sample_text)

print(f"Embedding shape: {embedding.shape}")
print(f"Embedding dtype: {embedding.dtype}")
print(f"First 5 values: {embedding[:5]}")

expected_dim = 768
if embedding.shape[0] == expected_dim:
    print(f"\nPASS: embedding dimension matches expected {expected_dim}.")
else:
    print(f"\nFAIL: expected dimension {expected_dim}, got {embedding.shape[0]}.")