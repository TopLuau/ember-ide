# Ember IDE installer for Windows (runs the Setup program silently).
#   $env:EMBER_SITE="https://your-site"; irm https://your-site/install.ps1 | iex
# Without EMBER_SITE it downloads the latest GitHub release instead.
$ErrorActionPreference = "Stop"
$repo = if ($env:EMBER_REPO) { $env:EMBER_REPO } else { "topluau/ember-ide" }

if ($env:EMBER_SITE) {
  $m = Invoke-RestMethod "$($env:EMBER_SITE)/downloads/manifest.json"
  $f = $m.files | Where-Object { $_.name -like "Ember-IDE-Setup-*.exe" } | Select-Object -First 1
  if (-not $f) { throw "No Windows installer found on $($env:EMBER_SITE)" }
  $name = $f.name
  $url = "$($env:EMBER_SITE)/downloads/$name"
} else {
  $rel = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
  $asset = $rel.assets | Where-Object { $_.name -like "Ember-IDE-Setup-*.exe" } | Select-Object -First 1
  if (-not $asset) { throw "No Windows installer found in the latest release of $repo" }
  $name = $asset.name
  $url = $asset.browser_download_url
}
$out = Join-Path $env:TEMP $name
Write-Host "Downloading $name ..."
Invoke-WebRequest $url -OutFile $out
Start-Process $out -ArgumentList "/SILENT" -Wait
Write-Host "Ember IDE installed."
