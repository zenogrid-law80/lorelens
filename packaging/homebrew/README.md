# Homebrew Cask 제출 준비

LoreLens는 `.app`을 배포하는 GUI 앱이므로 Cask로 등록합니다.
`lorelens.rb.in`은 제출용 템플릿이며, 아직 설치 가능한 Cask가 아닙니다.

## 2026-09-09 확인 결과

- 공개 최신 릴리스 `v0.1.2`에는 Windows MSI만 있습니다.
- 로컬 DMG와 앱은 `0.1.0`이며 앱 서명은 ad-hoc입니다. 이 파일을 `0.1.2`로 이름만 바꾸어 배포하면 안 됩니다.
- `build-macos.sh`도 ad-hoc 서명만 적용합니다. 공식 Cask는 Gatekeeper 검사를 통과해야 합니다.
- 저장소는 2026-09-07 생성되었으며 stars/forks/watchers가 모두 0입니다. 공식 정책상 생성 30일 미만 저장소는 보통 대상이 아니며, 소유자의 직접 제출에는 일반적으로 90 forks, 90 watchers, 225 stars 중 하나 또는 인정되는 예외 근거가 필요합니다.
- 따라서 공식 Homebrew PR은 아직 제출하지 않았습니다. 인지도 조건을 충족하기 전에는 별도 `homebrew-tap` 저장소로 배포할 수 있습니다.

## 배포 파일 준비

1. 배포할 버전의 소스에서 macOS release 앱을 빌드합니다.
2. 번들된 Lore CLI를 포함한 실행 파일과 앱을 Developer ID로 서명하고 공증합니다. 현재 빌드 스크립트는 이 과정을 구현하지 않습니다. 서명 후 `build-dmg.sh`를 다시 실행하면 앱을 재빌드하고 ad-hoc 서명하므로, 서명한 앱에서 별도로 DMG를 만들어야 합니다.
3. 공증한 최종 앱에 대해 `spctl --assess --type execute --verbose=4 /path/to/LoreLens.app`가 성공하는지 확인하고 최신 macOS에서 실행을 확인합니다.
4. 앱과 번들 CLI 각각에 `xcrun vtool -show-build`를 실행하여 최소 macOS 버전을 확인합니다. 템플릿의 Big Sur 값은 로컬 0.1.0 바이너리에서 확인한 값이므로 최종 릴리스에서도 확인해야 합니다. Apple Silicon만 선언하며 Intel 빌드는 가정하지 않습니다.
5. 최종 `LoreLens-0.1.2-macos-arm64.dmg`를 GitHub `v0.1.2` 릴리스에 업로드하고, 공개 URL에서 다시 내려받아 `shasum -a 256`을 계산합니다. 다른 버전을 배포하면 템플릿의 `version`도 변경합니다.
6. 템플릿을 `lorelens.rb`로 복사하고 SHA-256 자리표시자를 실제 값으로 교체합니다. `:no_check`로 대체하지 않습니다.

## 검증과 제출

공식 등록 조건을 충족하면 Homebrew/homebrew-cask 포크의 `Casks/l/lorelens.rb`에 완성한 Cask를 넣습니다. 해당 tap에서 다음 검사를 수행합니다.

```bash
brew style --cask lorelens
brew audit --new --online --cask lorelens
brew install --cask lorelens
```

설치한 앱을 실행해 확인한 후, 검증 결과를 포함하여 Homebrew/homebrew-cask에 PR을 제출합니다. 기존 수동 설치 앱이 있으면 테스트 시 이를 덮어쓰지 않도록 별도 테스트 환경을 사용합니다.

자체 tap으로 먼저 배포하는 경우 `zenogrid-law80/homebrew-tap`의 `Casks/lorelens.rb`에 완성한 파일을 게시합니다. 저장소와 파일을 게시한 뒤 사용할 명령은 다음과 같습니다.

```bash
brew install --cask zenogrid-law80/tap/lorelens
```

현재 이 tap의 생성이나 게시를 완료한 상태는 아닙니다.

공식 문서:
- [Adding Software to Homebrew](https://docs.brew.sh/Adding-Software-to-Homebrew)
- [Acceptable Casks](https://docs.brew.sh/Acceptable-Casks)
- [Package Acceptance Policy](https://docs.brew.sh/Package-Acceptance-Policy)
- [How to Create and Maintain a Tap](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)
