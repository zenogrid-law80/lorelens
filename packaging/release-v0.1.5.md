## 변경 사항

- Files 패널을 폴더 펼치기·접기와 들여쓰기를 지원하는 트리 구조로 변경했습니다.
- 왼쪽 목록의 다중 선택 작업을 보완했습니다. Stage, Unstage, Revert, Delete를 선택 항목에 적용하며 삭제 확인창에 대상 목록을 표시합니다.
- Changes에 필터와 전체 선택을 추가하고, 파일·폴더 아이콘 및 ACTION / STATE 컬럼으로 변경 내용과 스테이징 상태를 구분합니다.
- Changes 우클릭 메뉴는 현재 상태에서 가능한 작업만 제공합니다.

## 설치

Windows x64용 `LoreLens-0.1.5.msi`를 다운로드하세요. Lore CLI는 별도 설치하며, 앱에서 설치 안내 또는 Locate CLI를 사용할 수 있습니다.

## 검증 및 제한

- 테스트 40개 통과, Lore CLI 통합 테스트 1개 제외.
- Windows Release 빌드 및 MSI 생성. 실제 UI와 MSI 설치·업그레이드 테스트는 수행하지 않았습니다.
- 이번 릴리스는 Lore CLI의 누락 콘텐츠 또는 Address not found 문제를 수정하지 않습니다.
