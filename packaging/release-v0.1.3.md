## 변경 사항

- 파일 목록과 대기 중인 변경 목록에 영구 제거(Obliterate) 메뉴를 추가했습니다. 대상과 영향을 확인하고 `OBLITERATE`를 입력해야 실행됩니다.
- 현재 경로를 클립보드로 복사하는 기능을 추가했습니다.
- Windows에서 Lore CLI가 없을 때 WinGet 설치 안내를 제공합니다. CLI는 이번 MSI에 포함하지 않습니다.
- Windows MSI 아키텍처를 x64로 명시하고 설치 파일 버전을 Cargo.toml과 연동했습니다.

## 다운로드

Windows x64: `LoreLens-0.1.3.msi`를 다운로드해 설치하세요. 파일 검증용 SHA256은 `SHA256SUMS.txt`에 있습니다.

Lore CLI가 필요한 경우 앱의 설치 안내를 이용하거나, 별도로 설치한 CLI를 `Locate CLI…`로 지정할 수 있습니다.

## 확인 및 제한 사항

- Release 빌드 및 MSI 생성 완료. 테스트 32개 통과, 실제 Lore 저장소 통합 테스트 1개 제외.
- MSI 설치/업그레이드 테스트는 수행하지 않았습니다. 이번 릴리스에는 macOS 설치 파일을 포함하지 않습니다.
- Obliterate는 저장된 콘텐츠를 영구 제거하며 원격 저장소에도 영향을 줄 수 있습니다. 모든 과거 버전을 일괄 제거하는 기능은 아니며 Commit/Push는 별도로 수행해야 합니다.
- Lore CLI의 `Address not found` 및 누락 콘텐츠로 인한 Clone 실패는 이번 릴리스에서 수정되지 않았습니다.
