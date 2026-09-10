# LoreLens website

LoreLens 소개용 반응형 정적 사이트. Node.js가 있으면 외부 패키지 설치 없이 실행합니다.

```powershell
cd website
npm run dev
# http://127.0.0.1:4173
npm run build
```

배포 대상은 `dist/`의 정적 파일입니다. 하위 경로에서도 동작하도록 자산에 상대 경로를 사용합니다. `index.html`을 직접 열어 볼 수도 있습니다. 클립보드 복사는 localhost 또는 HTTPS에서 지원되며, 실패 시 수동 복사를 안내합니다.

제품 소개, 기능, 앱 스크린샷, 작업 흐름, Windows 빌드 안내를 제공합니다. 영어, 한국어, 중국어를 지원하며 브라우저 언어 또는 헤더의 언어 선택을 적용하고 선택값을 저장합니다. 상단 API Docs 메뉴에서 CLI 연결, 저장소 명령, JSON 이벤트 및 오류 처리 문서를 확인할 수 있습니다. 문서 내용은 `lorelens/src/backend/cli.rs`, `lorelens/src/backend.rs`, `lorelens/src/commands.rs`의 구현을 기준으로 작성했습니다. 가운데 이미지는 제공된 `dist/img.png`를 원본 `img.png`로 보관하여 사용합니다. 빌드할 때 `dist/img.png`로 복사됩니다. 다운로드 파일이나 미지원 OS를 제공하는 것으로 표시하지 않습니다.

참고: https://github.com/BiloxiStudios/loregui/tree/main/website 의 소개 → 기능 → 화면 → 설치 구성과 어두운 색상 체계를 참고했습니다. 원본 코드·이미지·브랜딩은 복사하지 않았습니다. LoreLens의 실제 기능에 맞춰 독립적으로 작성했습니다.
