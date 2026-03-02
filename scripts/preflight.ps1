$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "[preflight] openclaw-json-repair-kit"

$pythonCmd = $null
if (Get-Command py -ErrorAction SilentlyContinue) {
  $pythonCmd = "py"
} elseif (Get-Command python -ErrorAction SilentlyContinue) {
  $pythonCmd = "python"
} else {
  Write-Host "[error] Python not found. Install Python 3.10+ first."
  exit 1
}

$versionOutput = if ($pythonCmd -eq "py") { & py -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}')" } else { & python -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}')" }
Write-Host "[ok] Python: $pythonCmd ($versionOutput)"

$versionOk = if ($pythonCmd -eq "py") {
  & py -c "import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)"
  $LASTEXITCODE -eq 0
} else {
  & python -c "import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)"
  $LASTEXITCODE -eq 0
}

if (-not $versionOk) {
  Write-Host "[error] Python 3.10+ is required."
  exit 1
}

if (Test-Path ".venv\Scripts\Activate.ps1") {
  Write-Host "[ok] Virtual environment exists: .venv"
} else {
  Write-Host "[warn] .venv not found"
  if ($pythonCmd -eq "py") {
    Write-Host "      Next: py -m venv .venv"
  } else {
    Write-Host "      Next: python -m venv .venv"
  }
}

if (Test-Path "requirements.txt") {
  $missing = if ($pythonCmd -eq "py") {
    & py -c "import importlib.util; req=['pytest','httpx','respx','dotenv']; print(','.join([n for n in req if importlib.util.find_spec(n) is None]))"
  } else {
    & python -c "import importlib.util; req=['pytest','httpx','respx','dotenv']; print(','.join([n for n in req if importlib.util.find_spec(n) is None]))"
  }

  if ([string]::IsNullOrWhiteSpace($missing)) {
    Write-Host "[ok] Core dependencies appear installed"
  } else {
    Write-Host "[warn] Missing Python packages: $missing"
    if ($pythonCmd -eq "py") {
      Write-Host "      Next: py -m pip install -r requirements.txt"
    } else {
      Write-Host "      Next: python -m pip install -r requirements.txt"
    }
  }
} else {
  Write-Host "[warn] requirements.txt not found"
}

if (Test-Path ".env") {
  Write-Host "[ok] .env exists"
} elseif (Test-Path ".env.example") {
  Write-Host "[warn] .env missing"
  Write-Host "      Next: Copy-Item .env.example .env"
} else {
  Write-Host "[warn] .env.example not found"
}

Write-Host ""
Write-Host "[next] Suggested commands:"
if ($pythonCmd -eq "py") {
  Write-Host "  1) py -m venv .venv"
  Write-Host "  2) .\.venv\Scripts\Activate.ps1"
  Write-Host "  3) py -m pip install -r requirements.txt"
  Write-Host "  4) Copy-Item .env.example .env"
  Write-Host "  5) py -m kit tui"
} else {
  Write-Host "  1) python -m venv .venv"
  Write-Host "  2) .\.venv\Scripts\Activate.ps1"
  Write-Host "  3) python -m pip install -r requirements.txt"
  Write-Host "  4) Copy-Item .env.example .env"
  Write-Host "  5) python -m kit tui"
}
