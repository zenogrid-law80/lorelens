# LoreLens

Rust + GPUI로 만든 Windows용 로컬 Lore VCS 데스크톱 클라이언트입니다.
P4V의 작업 공간 / 변경 목록 / 제출 이력 / 상세 패널 구성을 참고했습니다.

## 실행

Rust stable 및 Visual Studio C++ Build Tools / Windows SDK가 필요합니다.

```powershell
cd C:\GitHub\lorelens
cargo run -- C:\GitHub\lore
```

빌드된 앱은 `target\debug\lorelens.exe`입니다. 실행 인자에 폴더를 지정하거나,
앱에서 **Open repository…**로 선택합니다. CLI 없이도 로컬 파일 탐색과 미리보기가 됩니다.

## Lore 연결

`C:\GitHub\lore`의 소스 및 CLI 이벤트 정의를 기준으로 구현했습니다.
이 소스 저장소 자체가 Lore 작업 공간이라는 뜻은 아닙니다.
실제 VCS 작업에는 기존 Lore 작업 공간과 Lore CLI가 필요합니다.

```powershell
cd C:\GitHub\lore
cargo build -p lore-client --bin lore
```

앱은 `LORELENS_LORE_BIN` 환경 변수 → 참조 저장소의 debug/release 실행 파일 →
PATH의 `lore` 순서로 찾습니다. **Locate CLI…**로 직접 지정할 수도 있습니다.
작업 공간을 연 뒤 **Refresh**를 누르면 `lore status --scan --json` 결과를 표시합니다.

## 사용 흐름

왼쪽 파일·폴더 우클릭 → `Move…`에서 Source를 확인하고 Destination에 이름을 포함한 목적지 경로를 입력합니다. 상대 경로는 저장소 루트 기준이며, 기존 대상 덮어쓰기와 저장소 밖 이동은 허용하지 않습니다. 이동 후 Refresh로 변경을 감지합니다.

왼쪽 목록에서 `↑`/`↓`로 항목을 선택하고, `→`로 선택한 폴더에 들어갑니다. `←`는 상위 폴더로 이동하며 저장소 루트에서는 이동하지 않습니다.

상단 `Push` 버튼은 현재 로그인 계정으로 현재 브랜치의 로컬 커밋을 원격에 전송합니다 (`lore push`). 완료 후 상태를 새로고침하며, 결과와 오류는 Details에 표시합니다. 전송 제한 시간은 30분입니다.

왼쪽 파일 항목을 더블클릭하면 Windows/macOS의 확장자 연결에 따라 기본 앱으로 엽니다. 한 번 클릭하면 미리보기를 표시하고, 폴더 더블클릭은 앱 안에서 폴더를 엽니다.

Clone 팝업의 마지막 `Repository URL`과 `Destination Path` 입력값은 편집 시 로컬 설정에 저장됩니다. 팝업을 취소하거나 앱을 종료해도 다음 실행에서 자동 복원됩니다.

`Clone repository…` 버튼에서 `Repository URL`과 `Destination Path`를 입력한 뒤 `Clone`을 누릅니다. 대상은 절대 경로이며 상위 폴더가 존재해야 합니다. 새 폴더 또는 빈 폴더에 복제하고, 성공하면 저장소를 자동으로 엽니다. 실패 원인은 Details에서 확인할 수 있습니다.

상단 `Account` 버튼으로 현재 저장소에서 사용하는 계정 정보를 Details에 표시합니다 (`lore auth info`). 로그인 성공 후에도 계정 정보를 자동 조회합니다. 인증 토큰은 표시하지 않습니다.

상단 `Sync` 버튼은 현재 저장소에서 `lore sync`를 실행합니다. 완료 후 파일 목록, 변경 상태, 잠금 표시와 브랜치 정보를 새로고침합니다. 실행 결과와 인증·충돌 오류는 Details에서 확인할 수 있습니다. `--reset`이나 `--force`는 적용하지 않습니다.

왼쪽 목록에서 `Ctrl`(macOS: `Cmd`)+클릭으로 파일·폴더를 추가 선택하거나 해제하고, `Shift`+클릭으로 범위를 선택합니다. 폴더는 더블클릭으로 엽니다. Stage/Unstage/Diff는 선택한 항목들에 적용되며, 우클릭 메뉴는 우클릭한 항목에 적용됩니다.

상단에서 `System`, `Light`, `Dark` 테마를 선택할 수 있습니다. 기본값인 `System`은 운영체제의 테마 변경을 따르며, 선택한 설정은 저장되어 다음 실행에도 유지됩니다.

왼쪽 파일 목록에서 우클릭하면 `gpui-component` 팝업 메뉴가 열립니다.
상태 조회에서 변경이 확인된 파일에는 `Discard`가 표시됩니다. 선택한 파일의 변경을 현재 revision으로 되돌리고 새로고침하며, 추적되지 않은 새 파일은 삭제합니다.
`Open Command Window Here`는 선택한 폴더(파일은 상위 폴더)에서 Windows의 `pwsh.exe` 또는 macOS의 Terminal을 엽니다.
`Show In` 하위 메뉴의 Explorer/Finder 항목은 선택한 경로를 파일 관리자에 표시합니다.
파일 우클릭 시 Lore 잠금 상태를 조회하며, 잠겨 있으면 `Unlock`, 잠겨 있지 않으면 `Lock`을 표시합니다. 선택하면 해당 파일의 잠금을 해제하거나 획득하고 상태를 새로고침합니다. 조회 실패 시 명령 로그에서 원인을 확인하고 메뉴를 다시 열어 재시도할 수 있습니다.

1. **Files**에서 폴더 탐색, 파일 검색 및 텍스트 미리보기.
2. **Refresh**로 수정·추가·삭제 파일을 스캔.
3. **Pending changes**에서 파일 선택 후 **Diff**, **Stage file**, **Unstage** 실행.
4. 메시지 입력 후 **Commit staged**로 현재 스테이징된 전체 변경을 로컬 커밋.
5. **Submitted revisions**, **Branches**, **File history**로 이력 조회.
6. 아래 **Details / Command log**에서 결과와 실패 원인 확인. **Copy**로 전체 결과 복사.

LoreLens는 CLI가 지원하는 기본 인자만 사용해 VCS 명령을 실행합니다.
원격 immutable 데이터를 읽는 명령에서 인증 오류가 발생하면 Lore CLI로 다시 로그인합니다.

```powershell
cd C:\GitHub\lore
..\lore\target\debug\lore.exe login lores://lore.zenogrid.co.kr:41337
```

로그인 후 LoreLens에서 **Refresh**를 다시 실행합니다.
Refresh의 `--scan`은 Lore의 dirty 상태를 갱신합니다.
커밋은 원격으로 push하지 않습니다. stage/unstage는 선택한 파일 하나에 적용됩니다.
프로세스는 셸을 거치지 않고 인자를 전달하며, 백그라운드에서 실행됩니다.
실행 중에는 중복 작업을 막고, 120초를 넘기면 종료 후 상태 재확인을 안내합니다.

## 현재 범위

- 네이티브 GPUI UI, 한글 IME와 선택/복사/붙여넣기를 지원하는 텍스트 입력.
- 로컬 폴더 탐색과 기존 Lore 작업 공간의 상태·스테이징·커밋·diff·이력·브랜치 조회.
- 이력과 브랜치는 CLI 텍스트 결과로 표시합니다.
- **Switch local branch…**로 로컬 브랜치를 전환하고, **New local branch / New remote branch**로 브랜치를 생성합니다.
- **Merge local branch…**에서 Source 브랜치를 선택하면 현재 브랜치(Target)로 병합합니다. 현재 브랜치는 선택 목록에서 제외됩니다. 충돌이 없으면 Lore가 자동으로 로컬 커밋하며, 원격 반영은 **Push**로 실행합니다. 결과와 충돌은 **Details / Command log**에서 확인하고, 충돌 해결은 Lore CLI에서 수행합니다. 병합 후 파일 상태를 자동으로 갱신합니다.
- 폴더별 탐색이며 `.git`, `.lore`, `target`은 숨깁니다. 한 번에 최대 2,000개 검색 결과를 표시합니다.
- 미리보기는 128 KiB, 상세 화면은 1,500줄, 명령 결과 수집은 16 MiB로 제한합니다.
- 왼쪽 패널은 브랜치 UI → 검색 → 파일 목록 순서로 고정됩니다. 오른쪽 탭을 바꿔도 파일 탐색을 계속할 수 있습니다.
- 최근 repository 10개와 선택한 Lore CLI 경로를 `%LOCALAPPDATA%\LoreLens\settings.json`에 저장합니다. 앱을 다시 실행하면 마지막으로 연 유효한 repository를 복원하며, 실행 인자로 지정한 경로가 우선합니다.
- 왼쪽 상단 **Recent ▾**에서 이전 repository를 다시 열 수 있습니다. 현재 브랜치 버튼을 누르면 gpui-component 팝업이 열립니다. Sync·Push·브랜치 생성, 로컬 브랜치별 전환·병합 하위 메뉴와 원격 브랜치 목록을 제공합니다. 현재 브랜치는 체크 표시합니다.

## 검증

```powershell
cargo fmt --check
cargo clippy -- -D warnings
cargo test 
cargo test -- --include-ignored  # Lore CLI가 있으면 실제 로컬 VCS 통합 테스트 포함
cargo build 
```

구조: `src/main.rs`(UI), `src/backend.rs`(CLI·JSON·파일 접근),
`src/input.rs`(GPUI 텍스트 입력).
GPUI 공식 문서: https://gpui.rs/

## External diff / merge tools

상단 Tools → Diff / Merge에서 idea, p4merge, TortoiseGitMerge를 선택합니다. 비교/병합은 하나의 도구 선택값과 실행 파일 경로를 공유합니다. 실행 인수는 Diff/Merge별로 유지됩니다. 선택값은 다음 실행에도 유지됩니다. 기존 설정의 선택값이 다르면 Diff 도구를 우선 사용합니다. PATH에서 실행 파일을 찾지 못하면 위치 선택 창이 열립니다. Tools 메뉴의 **Locate executable…**으로 직접 지정할 수도 있고, **Use PATH**로 지정 경로를 해제할 수 있습니다. 도구별 실행 파일 경로는 Diff/Merge에서 공유하며 다음 실행에도 유지됩니다. Windows에서는 .exe 또는 .com 파일(예: idea64.exe, rider64.exe)을 선택합니다. Diff 실행 중 경로를 지정하면 해당 비교를 이어서 실행합니다.

왼쪽 파일 우클릭 → Diff 또는 기존 Diff 버튼은 현재 리비전과 로컬 파일의 복사본을 비교합니다. p4merge와 TortoiseGitMerge는 요청한 4개 인수를 사용하며 결과도 임시 파일입니다. IDE가 기존 프로세스에서 파일을 열 수 있도록 비교 파일은 OS 임시 폴더의 lorelens-diff-*에 남습니다. 현재 리비전에 없는 신규 파일은 추출 오류가 표시됩니다.

Merge tools는 선택 및 명령 인수 프리셋을 저장합니다. 충돌 해결 실행은 아직 연결되지 않았습니다.

## 메뉴 배치

- Repository: 저장소 열기, Clone, 최근 저장소
- Changes: Stage, Unstage, Commit staged, File history, Pending push
- Tools: Diff / Merge 도구 선택, Locate executable, Use PATH, Locate Lore CLI
- View: 테마 선택, Command log
- Account: 로그인과 현재 계정 표시

두 번째 줄에는 저장소 선택, 브랜치, Refresh, Sync, Push를 배치합니다. 파일 우클릭 메뉴는 Diff(M 표시 파일만), Stage/Unstage, Discard, Move, 탐색기에서 보기, 터미널 열기, Lock/Unlock 순서입니다.

## 다국어 UI

**View → Language**(한국어: **보기 → 언어**)에서 English / 한국어를 선택합니다. 기본값은 영어이며 선택 즉시 화면에 적용되고, 설정 파일에 저장되어 재실행 시 복원됩니다.

- `i18n/en-US.json`: 기준 영문 카탈로그
- `i18n/ko-KR.json`: 한국어 카탈로그
- `lorelens/src/i18n.rs`: 번역 조회와 언어 전환, 변수 치환

카탈로그는 영문 문구를 키로 사용하는 평면 JSON입니다. `{count}`, `{path}` 같은 변수 이름은 번역에서도 유지해야 하며 문장 내 순서는 바꿀 수 있습니다. 누락된 번역은 영어로, 알 수 없는 언어 설정은 en-US로 대체됩니다. 파일은 빌드 시 포함되므로 배포할 때 별도 복사가 필요 없고, 수정 후에는 재빌드해야 합니다.

새 언어를 추가하려면 en-US.json을 복사해 번역하고 i18n.rs의 카탈로그 로딩 및 LOCALES 목록에 등록합니다. CLI 명령 인수, CLI 원문 출력·진단 로그, 파일 내용·경로와 외부 프로그램 이름은 번역하지 않습니다. 과거 작업 결과는 생성 당시의 언어를 유지할 수 있습니다.
