# macOS 앱 만들기

macOS에서 Xcode와 Metal Toolchain 설치 후 프로젝트 루트에서 실행합니다.

```bash
bash packaging/build-macos.sh
open dist/LoreLens.app
```

`dist/LoreLens.app`에 실행 파일, macOS Lore CLI, `favicon.ico`에서 생성한 다중 해상도 ICNS 아이콘을 포함합니다. 로컬 실행을 위한 ad-hoc 서명을 적용하며, 외부 배포용 Developer ID 서명과 공증은 별도입니다.

빠른 개발 빌드는 `bash packaging/build-macos.sh debug`로 만듭니다. `cargo run`으로 직접 실행해도 내장 아이콘을 사용해 Dock 아이콘을 설정합니다.

DMG 설치 이미지는 다음 명령으로 생성합니다. 앱을 Applications 폴더로 드래그해 설치할 수 있으며, 출력 파일명에 빌드한 Mac의 아키텍처가 포함됩니다.

```bash
bash packaging/build-dmg.sh
# Apple Silicon: dist/LoreLens-0.1.1-macos-arm64.dmg
```

# MSI 패키지 만들기

PowerShell에서 프로젝트 루트 기준으로 실행합니다.

```powershell
.\packaging\build-msi.ps1
```

스크립트는 Release 실행 파일을 빌드하고, 필요한 경우 WiX v4를 `.tools` 아래에 설치한 뒤 `dist\LoreLens-0.1.1.msi`를 생성합니다.

이미 `target\release\lorelens.exe`가 있다면 다음처럼 Rust 빌드를 건너뛸 수 있습니다.

```powershell
.\packaging\build-msi.ps1 -SkipBuild
```
