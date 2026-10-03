# ============================================================
# record-demo.ps1 — Graba la app y genera demo.gif para el README
# Uso: .\scripts\record-demo.ps1
# ============================================================

param(
    [int]$DurationSeconds = 30,    # Cuántos segundos grabar
    [int]$Width           = 1000,  # Ancho del área a capturar
    [int]$Height          = 700,   # Alto del área a capturar
    [int]$OffsetX         = 0,     # Posición X de la ventana (ajusta si hace falta)
    [int]$OffsetY         = 0,     # Posición Y de la ventana (ajusta si hace falta)
    [int]$Fps             = 15,    # FPS del GIF (15 es suficiente y pesa poco)
    [string]$OutGif       = "docs\demo.gif",
    [string]$TmpMp4       = "$env:TEMP\scrcpy-gui-demo-tmp.mp4"
)

$ffmpeg = "ffmpeg"

Write-Host ""
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  scrcpy GUI — Grabador de demo" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Abre la app (npm start) y pon la ventana visible." -ForegroundColor Yellow
Write-Host "Luego pulsa ENTER para empezar a grabar $DurationSeconds segundos." -ForegroundColor Yellow
Write-Host ""
Read-Host "Pulsa ENTER cuando estés listo"

Write-Host ""
Write-Host "▶  Grabando $DurationSeconds segundos..." -ForegroundColor Green
Write-Host "   Mueve la app, cambia ajustes, muestra las funciones." -ForegroundColor Gray
Write-Host ""

# 1) Grabar pantalla → MP4 temporal
& $ffmpeg -y `
    -f gdigrab `
    -framerate $Fps `
    -offset_x $OffsetX `
    -offset_y $OffsetY `
    -video_size "${Width}x${Height}" `
    -i desktop `
    -t $DurationSeconds `
    -vcodec libx264 `
    -pix_fmt yuv420p `
    $TmpMp4

if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Error al grabar. Asegúrate de que ffmpeg esté en el PATH." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "🔄 Convirtiendo a GIF optimizado..." -ForegroundColor Cyan

# 2) Generar paleta optimizada
$palette = "$env:TEMP\scrcpy-gui-palette.png"
& $ffmpeg -y -i $TmpMp4 `
    -vf "fps=$Fps,scale=800:-1:flags=lanczos,palettegen=stats_mode=diff" `
    $palette

# 3) Aplicar paleta → GIF final
& $ffmpeg -y -i $TmpMp4 -i $palette `
    -filter_complex "fps=$Fps,scale=800:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5" `
    $OutGif

# 4) Limpiar temporales
Remove-Item $TmpMp4 -ErrorAction SilentlyContinue
Remove-Item $palette -ErrorAction SilentlyContinue

if (Test-Path $OutGif) {
    $sizeMB = [math]::Round((Get-Item $OutGif).Length / 1MB, 1)
    Write-Host ""
    Write-Host "✅ GIF generado: $OutGif ($sizeMB MB)" -ForegroundColor Green
    Write-Host ""
    Write-Host "Próximos pasos:" -ForegroundColor Yellow
    Write-Host "  1. Abre docs\demo.gif y verifica que se ve bien" -ForegroundColor White
    Write-Host "  2. git add docs/demo.gif" -ForegroundColor White
    Write-Host "  3. git commit -m 'docs: add demo GIF'" -ForegroundColor White
    Write-Host "  4. git push" -ForegroundColor White
    Write-Host "  5. El README ya enlaza al GIF automáticamente" -ForegroundColor White
} else {
    Write-Host "❌ No se generó el GIF. Revisa los errores arriba." -ForegroundColor Red
}
