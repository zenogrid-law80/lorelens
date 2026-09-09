# LoreLens WinGet 패키지

패키지 ID: `zenogrid.LoreLens`

등록 PR: https://github.com/microsoft/winget-pkgs/pull/431691

세 매니페스트는 Microsoft 공식 1.6.0 JSON 스키마 검증을 통과했습니다. 공개 등록은 PR 검토 및 병합 대기 중입니다.

`manifests/z/zenogrid/LoreLens/0.1.2`에는 공개된 GitHub v0.1.2 MSI를 참조하는 제출용 매니페스트가 있습니다. SHA256은 GitHub 릴리스 asset digest와 로컬 MSI에서 확인했습니다. ProductCode와 표시 버전은 MSI 데이터베이스에서 추출했습니다.

## 검증 및 설치

WinGet이 설치된 Windows에서 프로젝트 루트 기준으로 실행합니다.

```powershell
winget validate --manifest .\packaging\winget\manifests\z\zenogrid\LoreLens\0.1.2
```

로컬 설치 테스트는 관리자 터미널에서 로컬 매니페스트 기능을 활성화한 후 실행합니다. 실제 프로그램을 설치하므로 테스트 PC 또는 VM을 권장합니다.

```powershell
winget settings --enable LocalManifestFiles
winget install --manifest .\packaging\winget\manifests\z\zenogrid\LoreLens\0.1.2
```

현재 작성 환경에는 WinGet이 없어 CLI 검증과 설치 테스트는 수행하지 않았습니다. v0.1.2 MSI의 SummaryInformation Template은 `Intel;0`입니다. 매니페스트는 앱의 x64 대상에 맞춰 x64로 제한했지만, 제출 전 Windows x64에서 설치/제거를 확인해야 합니다. 다음 MSI 빌드에서는 WiX에 `-arch x64`를 명시하여 패키지 아키텍처도 일치시키세요. 이미 배포된 MSI를 다시 빌드해 교체하면 해시와 ProductCode가 달라질 수 있으므로 새 버전으로 배포하세요.

## 커뮤니티 저장소 등록

1. https://github.com/microsoft/winget-pkgs 를 fork하고 clone합니다.
2. 이 폴더의 `manifests` 디렉터리를 fork 저장소 루트에 복사합니다.
3. 검증 및 설치 테스트 후 PR을 생성합니다.
4. PR 병합 및 소스 반영 후 아래 명령으로 설치할 수 있습니다.

```powershell
winget install --exact --id zenogrid.LoreLens
```

매니페스트를 로컬에 만드는 것만으로 WinGet 공개 저장소에 등록되지는 않습니다.

새 버전마다 버전 디렉터리, 세 파일의 PackageVersion, InstallerUrl, InstallerSha256, MSI ProductCode 및 DisplayVersion을 갱신합니다. 해시는 최종 업로드한 MSI 기준으로 계산합니다.

공식 안내: https://learn.microsoft.com/windows/package-manager/package/manifest
