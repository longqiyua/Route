Add-Type -AssemblyName System.Drawing

function Make-Png($path, $size) {
  $bmp = New-Object System.Drawing.Bitmap $size, $size
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.Clear([System.Drawing.Color]::FromArgb(110, 168, 254))
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(15, 17, 21))
  $font = New-Object System.Drawing.Font("Segoe UI", ($size * 0.5), [System.Drawing.FontStyle]::Bold)
  $sf = New-Object System.Drawing.StringFormat
  $sf.Alignment = [System.Drawing.StringAlignment]::Center
  $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
  $rect = New-Object System.Drawing.RectangleF 0, 0, $size, $size
  $g.DrawString("R", $font, $brush, $rect, $sf)
  $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  Write-Host "Created: $path"
}

Make-Png "crates/route-tauri/icons/32x32.png" 32
Make-Png "crates/route-tauri/icons/128x128.png" 128
Make-Png "crates/route-tauri/icons/128x128@2x.png" 256

# ICO containing the 32x32 PNG
$png = [System.IO.File]::ReadAllBytes("crates/route-tauri/icons/32x32.png")
$ico = New-Object System.Collections.Generic.List[byte]
$ico.AddRange([byte[]](0x00,0x00, 0x01,0x00, 0x01,0x00))
$ico.Add(32)
$ico.Add(32)
$ico.Add(0)
$ico.Add(0)
$ico.Add(1)
$ico.Add(0)
$ico.Add(32)
$ico.Add(0)
$ico.AddRange([BitConverter]::GetBytes([int32]$png.Length))
$ico.AddRange([BitConverter]::GetBytes([int32]22))
$ico.AddRange($png)
[System.IO.File]::WriteAllBytes("crates/route-tauri/icons/icon.ico", $ico.ToArray())
Write-Host "Created: icon.ico (size $($ico.Count))"

# ICNS using the 128 PNG
$png128 = [System.IO.File]::ReadAllBytes("crates/route-tauri/icons/128x128.png")
$icns = New-Object System.Collections.Generic.List[byte]
$icns.AddRange([byte[]](0x69,0x63,0x6E,0x73))
$totalLen = 8 + 8 + $png128.Length
$totalBytes = [BitConverter]::GetBytes([int32]$totalLen)
[Array]::Reverse($totalBytes)
$icns.AddRange($totalBytes)
$icns.AddRange([byte[]](0x69,0x63,0x30,0x37))
$innerLen = 8 + $png128.Length
$innerBytes = [BitConverter]::GetBytes([int32]$innerLen)
[Array]::Reverse($innerBytes)
$icns.AddRange($innerBytes)
$icns.AddRange($png128)
[System.IO.File]::WriteAllBytes("crates/route-tauri/icons/icon.icns", $icns.ToArray())
Write-Host "Created: icon.icns (size $($icns.Count))"
