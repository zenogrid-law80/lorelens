# MSI 패키지 만들기

PowerShell에서 프로젝트 루트 기준으로 실행합니다.

```powershell
.\packaging\build-msi.ps1
```

스크립트는 Release 실행 파일을 빌드하고, 필요한 경우 WiX v4를 `.tools` 아래에 설치한 뒤 `dist\LoreLens-0.1.0.msi`를 생성합니다.

이미 `target\release\lorelens.exe`가 있다면 다음처럼 Rust 빌드를 건너뛸 수 있습니다.

```powershell
.\packaging\build-msi.ps1 -SkipBuild
```
