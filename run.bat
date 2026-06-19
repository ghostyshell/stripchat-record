@echo off
setlocal
cd /d "%~dp0"

if not exist ".venv\Scripts\python.exe" (
  python -m venv .venv
  if errorlevel 1 (
    py -3 -m venv .venv
    if errorlevel 1 (
      echo Failed to create virtual environment. Install Python 3.10+ and try again.
      exit /b 1
    )
  )
  .venv\Scripts\pip install -r requirements.txt
  .venv\Scripts\python -m playwright install chromium
)

.venv\Scripts\python record_webrtc.py %*
