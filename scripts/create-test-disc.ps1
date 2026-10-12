<#
.SYNOPSIS
    Generates a synthetic Audio CD test image (test_disc.cue + test_disc.bin) for OxiDisc.
.DESCRIPTION
    Creates a virtual Red Book Audio CD image with synthetic PCM audio matching the official MusicBrainz test TOC.
.PARAMETER Fast
    Generates smaller tracks for faster test runs.
#>

param(
    [switch]$Fast
)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$OutputDir = Join-Path (Split-Path -Parent $ScriptDir) "test_data"
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$BinPath = Join-Path $OutputDir "test_disc.bin"
$CuePath = Join-Path $OutputDir "test_disc.cue"

Write-Host "Generating synthetic Audio CD image in: $OutputDir" -ForegroundColor Green
$Scale = if ($Fast) { 0.02 } else { 1.0 }

$Tracks = @(
    @{ Num = 1; Name = "Overture in C Major"; Freq = 440.0; Sectors = [int](15213 * $Scale) },
    @{ Num = 2; Name = "Harmonic Pulse"; Freq = 554.37; Sectors = [int](16951 * $Scale) },
    @{ Num = 3; Name = "Acoustic Reflection"; Freq = 659.25; Sectors = [int](14278 * $Scale) },
    @{ Num = 4; Name = "Binary Rhythm"; Freq = 880.0; Sectors = [int](16822 * $Scale) },
    @{ Num = 5; Name = "Spectral Waves"; Freq = 987.77; Sectors = [int](17075 * $Scale) },
    @{ Num = 6; Name = "Finale & Lead-Out"; Freq = 523.25; Sectors = [int](14973 * $Scale) }
)

$SampleRate = 44100
$FramesPerSector = 588 # 2352 bytes per sector

$CueLines = @('FILE "test_disc.bin" BINARY')
$CurrentSector = 0

$FileStream = [System.IO.File]::Create($BinPath)
$BinaryWriter = New-Object System.IO.BinaryWriter($FileStream)

try {
    foreach ($t in $Tracks) {
        $trackSectors = [Math]::Max(150, $t.Sectors)
        Write-Host "  Generating Track $($t.Num): '$($t.Name)' ($($t.Freq) Hz, $trackSectors sectors)..."

        $totalFrames = $trackSectors * $FramesPerSector
        $freq = [double]$t.Freq
        $twoPiF = 2.0 * [Math]::PI * $freq
        $twoPiFR = 2.0 * [Math]::PI * ($freq * 1.002)

        for ($i = 0; $i -lt $totalFrames; $i++) {
            $time = $i / [double]$SampleRate
            $sLeft = [Math]::Sin($twoPiF * $time) * 0.6 + [Math]::Sin($twoPiF * 2.0 * $time) * 0.15
            $sRight = [Math]::Sin($twoPiFR * $time) * 0.6 + [Math]::Sin($twoPiFR * 2.0 * $time) * 0.15

            $iLeft = [int16][Math]::Max(-32767, [Math]::Min(32767, $sLeft * 24000.0))
            $iRight = [int16][Math]::Max(-32767, [Math]::Min(32767, $sRight * 24000.0))

            $BinaryWriter.Write($iLeft)
            $BinaryWriter.Write($iRight)
        }

        $m = [int]($CurrentSector / (60 * 75))
        $s = [int](($CurrentSector / 75) % 60)
        $f = [int]($CurrentSector % 75)

        $CueLines += "  TRACK {0:D2} AUDIO" -f $t.Num
        $CueLines += "    TITLE `"$($t.Name)`""
        $CueLines += "    PERFORMER `"OxiDisc Audio Lab`""
        $CueLines += "    INDEX 01 {0:D2}:{1:D2}:{2:D2}" -f $m, $s, $f

        $CurrentSector += $trackSectors
    }
} finally {
    $BinaryWriter.Close()
    $FileStream.Close()
}

$CueLines -join "`r`n" | Set-Content -Path $CuePath -Encoding utf8

$fileSizeMb = ((Get-Item $BinPath).Length / 1MB).ToString("F2")
Write-Host "`nSuccessfully generated Audio CD test disc:" -ForegroundColor Green
Write-Host "  CUE Sheet: $CuePath"
Write-Host "  BIN Image: $BinPath ($fileSizeMb MB)"
Write-Host "`nUsage options:"
Write-Host "  1. Hardware-Free Mock Mode directly in OxiDisc:"
Write-Host "     cargo run --features egui-ui -- --mock-cd" -ForegroundColor Cyan
Write-Host "  2. Mount using virtual CD software (WinCDEmu / Virtual CloneDrive / cdemu):"
Write-Host "     Mount '$CuePath' as a virtual drive letter and point OxiDisc to it."
