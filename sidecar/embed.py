"""
embed.py — Long-lived Python embedding sidecar for LoomisCLI.
Communicates via newline-delimited JSON over stdin/stdout.

Architecture constraints:
- Narrow scope: loads jina-embeddings-v2-base-code once at startup, embeds query text on request.
- No LanceDB access, no reranking, no LLM calls.
- HF_HOME must be set BEFORE sentence_transformers/transformers import.
- Protocol schema version: "v": 1
"""

import os
import sys
from pathlib import Path

# HF_HOME must point to <project_root>/models and be set BEFORE any transformers import.
# Since this file lives in sidecar/embed.py, .parent.parent resolves to <project_root>.
PROJECT_ROOT = Path(__file__).resolve().parent.parent
CACHE_DIR = PROJECT_ROOT / "models"
os.environ["HF_HOME"] = str(CACHE_DIR)

import json
from sentence_transformers import SentenceTransformer

MODEL_NAME = "jinaai/jina-embeddings-v2-base-code"

def main():
    # Ensure stdout line buffering
    sys.stdout.reconfigure(line_buffering=True)
    sys.stderr.reconfigure(line_buffering=True)

    try:
        # Load model once at startup
        model = SentenceTransformer(MODEL_NAME, trust_remote_code=True)
    except Exception as e:
        err_payload = {
            "v": 1,
            "status": "error",
            "error": f"Failed to load model {MODEL_NAME}: {e}"
        }
        sys.stdout.write(json.dumps(err_payload) + "\n")
        sys.stdout.flush()
        sys.exit(1)

    # Handshake / ready signal
    ready_payload = {
        "v": 1,
        "status": "ready"
    }
    sys.stdout.write(json.dumps(ready_payload) + "\n")
    sys.stdout.flush()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        req_id = None
        try:
            req = json.loads(line)
            req_id = req.get("id")
            version = req.get("v")

            if version != 1:
                resp = {
                    "v": 1,
                    "id": req_id,
                    "status": "error",
                    "error": f"Unsupported protocol version: {version}. Expected 1."
                }
                sys.stdout.write(json.dumps(resp) + "\n")
                sys.stdout.flush()
                continue

            action = req.get("action", "embed")

            if action == "ping":
                resp = {
                    "v": 1,
                    "id": req_id,
                    "status": "ok",
                    "action": "pong"
                }
            elif action == "embed":
                text = req.get("text")
                if text is None:
                    resp = {
                        "v": 1,
                        "id": req_id,
                        "status": "error",
                        "error": "Missing 'text' field for embed action"
                    }
                else:
                    embedding = model.encode(text)
                    resp = {
                        "v": 1,
                        "id": req_id,
                        "status": "ok",
                        "embedding": embedding.tolist()
                    }
            else:
                resp = {
                    "v": 1,
                    "id": req_id,
                    "status": "error",
                    "error": f"Unknown action: '{action}'"
                }

            sys.stdout.write(json.dumps(resp) + "\n")
            sys.stdout.flush()

        except json.JSONDecodeError as e:
            resp = {
                "v": 1,
                "id": req_id,
                "status": "error",
                "error": f"Malformed JSON: {e}"
            }
            sys.stdout.write(json.dumps(resp) + "\n")
            sys.stdout.flush()
        except Exception as e:
            resp = {
                "v": 1,
                "id": req_id,
                "status": "error",
                "error": f"Internal error during embedding: {e}"
            }
            sys.stdout.write(json.dumps(resp) + "\n")
            sys.stdout.flush()

if __name__ == "__main__":
    main()
