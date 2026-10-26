# Anemobit

[English](README.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md) | **한국어**

Anemobit은 SNMP(v2c 또는 v3)로 장치 인터페이스의 트래픽을 실시간으로 보여 줍니다. 표준 IF-MIB와 64비트 인터페이스 카운터를 지원하는 장치라면 사용할 수 있습니다. macOS, iOS / iPadOS, Windows, Linux 각 플랫폼마다 네이티브 앱이 있습니다.

<p align="center">
  <img src="Screenshots/macos.png" width="720" alt="Anemobit macOS 버전">
</p>

## 기능

- 하나 이상의 인터페이스의 다운로드 및 업로드 속도를 실시간으로 표시하며, 인터페이스마다 게이지와 최근 5분 그래프를 제공합니다.
- 현재 속도, 5분 평균, 5분 최고
- 지정한 인터페이스 하나의 속도를 항상 표시: 메뉴 막대(macOS), 플로팅 창과 시스템 트레이(Windows), 시스템 트레이와 상단 표시줄(Linux)
- SNMPv2c, 그리고 인증(MD5, SHA1, SHA224, SHA256, SHA384, SHA512)과 암호화(DES, AES-128)를 지원하는 SNMPv3(Windows 버전은 SHA224 미지원)
- 장치를 검색해 인터페이스를 찾고 이름으로 선택
- 속도를 bit(Mbps) 또는 byte(MB/s)로 표시하며, 1초, 5초 또는 10초마다 새로 고침
- community와 SNMPv3 비밀번호는 시스템의 보안 저장소에 저장
- 영어, 번체 중국어, 일본어, 한국어 지원(기본적으로 시스템 언어를 따르며, 설정에서 선택 가능)
- 분석, 광고, 추적 기능이 없으며, 입력한 장치에만 연결합니다.

## 스크린샷

| iOS | Windows | Linux |
|:-:|:-:|:-:|
| <img src="Screenshots/iOS.png" width="160" alt="Anemobit iOS 버전"> | <img src="Screenshots/Windows.png" width="280" alt="Anemobit Windows 버전"> | <img src="Screenshots/Linux.png" width="280" alt="Anemobit Linux 버전"> |

## 다운로드

| 플랫폼 | 요구 사항 | 다운로드 |
|---|---|---|
| macOS | macOS 27 | Mac App Store(출시 예정) · [GitHub Releases](https://github.com/Noctqira/Anemobit/releases)의 DMG |
| iOS / iPadOS | iOS 27 / iPadOS 27 | App Store(출시 예정) |
| Windows | Windows 11, x64 또는 ARM64, [.NET 10 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/10.0) 필요 | [GitHub Releases](https://github.com/Noctqira/Anemobit/releases)의 MSI 설치 프로그램 |
| Linux | Ubuntu 26.04 또는 Fedora 44, x86_64 또는 ARM64 | [GitHub Releases](https://github.com/Noctqira/Anemobit/releases)의 .deb / .rpm |

## 장치 설정

Anemobit은 장치의 SNMP 에이전트를 통해 인터페이스 카운터를 읽습니다. 설정 방법은 장치마다 다르므로 자세한 내용은 장치 설명서를 참고하세요.

1. **SNMP 켜기**: 장치에서 SNMP를 켭니다(SNMP 에이전트 또는 SNMP 서비스라고도 합니다).
2. **보안 선택**:
   - **SNMPv2c**: community 이름을 설정합니다. 기본값인 `public`은 사용하지 마세요.
   - **SNMPv3**: 사용자를 만들고 인증 프로토콜과 비밀번호를 설정합니다. 필요하면 암호화 프로토콜과 비밀번호도 설정합니다. 비밀번호는 8자 이상이어야 합니다.
3. **접근 제한**: 읽기 전용 권한이면 충분합니다. 장치가 지원하면 Anemobit을 실행하는 컴퓨터나 휴대폰의 SNMP 요청만 받도록 설정합니다.
4. **통신 허용**: 장치의 방화벽에서 로컬 네트워크의 SNMP(UDP 포트 161)를 허용합니다.

그런 다음 Anemobit의 설정을 열고 장치 주소와 같은 보안 설정을 입력한 뒤, 인터페이스를 검색해 모니터링할 인터페이스를 선택합니다. 장치는 IF-MIB의 64비트 카운터(`ifHCInOctets` / `ifHCOutOctets`)를 제공해야 합니다.

## 소스에서 빌드

각 플랫폼은 자체 네이티브 도구로 따로 빌드합니다.

| 폴더 | 플랫폼 | 기술 | 빌드 방법(영어) |
|---|---|---|---|
| `Darwin/` | macOS, iOS / iPadOS | Swift + SwiftUI | [Darwin/README.md](Darwin/README.md) |
| `Windows/` | Windows | C# + WPF(.NET 10) | [Windows/README.md](Windows/README.md) |
| `Linux/` | Linux | Rust + GTK4 + libadwaita | [Linux/README.md](Linux/README.md) |

## 라이선스

Anemobit은 [Apache License 2.0](LICENSE)으로 배포되며, 저작권은 © 2026 Noctqira에 있습니다([NOTICE](NOTICE) 참고). 'Anemobit' 이름과 아이콘은 라이선스 범위에 포함되지 않습니다. 수정한 버전을 배포할 때는 다른 이름과 아이콘을 사용하세요. 그 밖에 바꿔야 할 부분은 각 플랫폼의 README에 정리되어 있습니다.

개인정보 처리방침: [PRIVACY.md](PRIVACY.md)
