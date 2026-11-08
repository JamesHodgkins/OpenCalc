# Linux file type

Untested on a real desktop yet. To make `.ocx` files open in OpenCalc for one user:

```
install -Dm755 target/release/opencalc ~/.local/bin/opencalc
install -Dm644 packaging/linux/opencalc.desktop ~/.local/share/applications/opencalc.desktop
install -Dm644 packaging/linux/opencalc-mime.xml ~/.local/share/mime/packages/opencalc.xml
install -Dm644 assets/icons/opencalc.png ~/.local/share/icons/hicolor/256x256/apps/opencalc.png
install -Dm644 assets/icons/ocx.png ~/.local/share/icons/hicolor/256x256/mimetypes/application-vnd.opencalc.worksheet.png
update-mime-database ~/.local/share/mime
update-desktop-database ~/.local/share/applications
```
