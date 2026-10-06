# macOS 앱 만들기

Homebrew Cask 제출 준비와 배포 전제 조건은 [homebrew/README.md](homebrew/README.md)를 참고하세요.

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
# Apple Silicon: dist/LoreLens-0.1.2-macos-arm64.dmg
```

# MSI 패키지 만들기

# 최신 LoreLens 설치

인터넷에서 최신 안정 버전 MSI를 내려받아 설치하려면 PowerShell에서 실행합니다.

```powershell
.\packaging\install-lorelens.ps1
```

무인 설치는 다음과 같이 실행합니다.

```powershell
.\packaging\install-lorelens.ps1 -Silent
```

PowerShell에서 프로젝트 루트 기준으로 실행합니다.

```powershell
.\packaging\build-msi.ps1
```

스크립트는 Release 실행 파일을 빌드하고, 필요한 경우 WiX 6.0.2와 같은 버전의 Util 확장을 `.tools` 아래에 설치한 뒤 Cargo.toml 버전으로 x64 MSI(`dist\LoreLens-<버전>.msi`)를 생성합니다. Lore CLI는 포함하지 않으며, 앱의 설치 안내 또는 `Locate CLI…`로 연결합니다.

설치·업데이트 시 LoreLens가 실행 중이면 기존 버전을 제거하거나 파일을 교체하기 전에 종료 안내를 표시합니다. 진행 중인 작업을 마친 뒤 앱을 닫고 **Retry(다시 시도)**를 누르거나, **Ignore(무시)**를 눌러 설치 프로그램이 정상 종료를 요청하게 할 수 있습니다. **Abort(중단)**는 설치를 취소합니다. 무인 설치에서는 안내 없이 정상 종료를 요청합니다. 종료를 요청한 뒤에도 15초 내에 앱이 닫히지 않으면 설치를 중단하며, 앱을 직접 닫은 뒤 다시 설치해야 합니다.

이미 `target\release\lorelens.exe`가 있다면 다음처럼 Rust 빌드를 건너뛸 수 있습니다.

```powershell
.\packaging\build-msi.ps1 -SkipBuild
```

# 앱에서 자체 업데이트

**옵션 → 업데이트**에서 현재 버전, 새 버전과 변경 이력을 확인합니다. **시작 시 업데이트 확인**은 기본으로 켜져 있으며 앱을 시작할 때 최신 안정 릴리스가 있는지만 확인합니다. 다운로드와 설치는 자동으로 실행하지 않습니다. **업데이트 확인**으로 언제든 다시 확인할 수 있습니다.

Windows x64에서는 **다운로드 및 설치**를 누르면 공식 GitHub 릴리스의 MSI를 내려받고 SHA-256을 검증한 뒤 설치 프로그램을 실행합니다. 설치 프로그램의 종료 안내에서 앱을 닫고 **Retry(다시 시도)**를 누르거나 **Ignore(무시)**를 눌러 정상 종료 후 업데이트를 진행하세요. macOS, Linux와 그 밖의 플랫폼에서는 릴리스 페이지를 열어 해당 플랫폼의 설치 파일을 내려받습니다.
