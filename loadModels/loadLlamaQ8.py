"""
loadLlamaQ8.py — downloads Llama-3.2-1B-Instruct-Q8_0.gguf into the project's shared
models\\ directory (same location loadJina.py uses).

This is a download-only script. It does NOT load/run the model — that's llama-server's
job, started separately and pointed at the downloaded file.

Location: LoomisCLI\\loadModels\\loadLlamaQ8.py
Usage (with venv activated, from project root):
    python loadModels\\loadLlamaQ8.py
"""

import os
from pathlib import Path

# Same resolution pattern as loadJina.py — portable, not hardcoded, and self-contained
# (doesn't rely on any $env:HF_HOME set in the shell session).
MODELS_DIR = Path(__file__).parent.parent / "models"
os.environ["HF_HOME"] = str(MODELS_DIR)

from huggingface_hub import hf_hub_download

REPO_ID = "bartowski/Llama-3.2-1B-Instruct-GGUF"
FILENAME = "Llama-3.2-1B-Instruct-Q8_0.gguf"

print(f"Downloading {FILENAME} from {REPO_ID} into {MODELS_DIR} ...")

downloaded_path = hf_hub_download(
    repo_id=REPO_ID,
    filename=FILENAME,
    local_dir=str(MODELS_DIR),
)

print(f"Done. File at: {downloaded_path}")