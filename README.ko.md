# NMT Code

[English](README.md) | 한국어

NMT Code(needmoretruth code)는 요즘 폰과 컴퓨터의 카메라로 읽는 2차원 코드입니다. 같은 내용을
QR 코드보다 적은 모듈에 담습니다.

![QR 코드 아래에 있는, 이 저장소 주소를 담은 NMT Code](docs/images/example-url.png)

아래쪽 NMT Code에는 `https://github.com/needmoretruth/nmtcode`가 들어 있습니다. 위쪽 QR 코드에도 같은
주소가 들어 있어서, QR 코드만 읽는 폰으로 찍으면 이 페이지로 옵니다. 이 QR 코드는 `--qr`로 붙이고,
인쇄용 코드(`--profile print`)에는 처음부터 붙습니다.

> **버전 0.0.1**은 흑백 코드를 PNG나 SVG로 만들고, 카메라 사진 · 화면 캡처 · 이미지 파일(PNG, JPEG)에서
> 읽습니다. CLI, 웹 페이지, Rust 라이브러리로 씁니다. 색 코드와 여러 장으로 파일 옮기기는 아직 만들고
> 있습니다. 0.0.1로 만든 코드는 뒤의 모든 0.x 버전에서도 읽힙니다.

## QR 코드와 크기 비교

| 내용 | 바이트 | NMT Code | QR 코드 |
|---|---:|---:|---:|
| `https://github.com/needmoretruth/nmtcode` | 40 | 20 × 28 = 560 | 29 × 29 = 841 |
| 한국어 문장, UTF-8로 84바이트 | 84 | 24 × 36 = 864 | 37 × 37 = 1,369 (UTF-8) · 33 × 33 = 1,089 (EUC-KR) |
| 87바이트 영어 문장 | 87 | 24 × 36 = 864 | 37 × 37 = 1,369 |
| 숫자 50개 | 50 | 20 × 28 = 560 | 25 × 25 = 625 |

- 두 코드 모두 가장 낮은 오류 정정 수준입니다. 바이트의 약 7.5%(NMT Code)와 7%(QR 코드)를
  되살립니다. QR 코드는 그 내용이 들어가는 가장 작은 버전입니다. QR 리더 중에는 한국어를 UTF-8일
  때만 제대로 읽는 것이 많습니다.
- 모듈 수에 여백은 넣지 않았습니다. NMT Code는 네 변에 2모듈씩, QR 코드는 4모듈씩 필요합니다.
- NMT Code는 긴 변이 짧은 변의 두 배를 넘지 않는 직사각형이라 정사각형이 아닐 때가 있습니다.
- 모듈 수는 코드만 센 것입니다. `--qr`로 옆에 QR 코드를 붙이면 그림은 같은 내용의 QR 코드보다
  커집니다.
- 2026-09-29에 nmtcode 0.0.1, zxing-cpp 3.1.1, segno 1.6.6으로 쟀습니다.

## 카메라 사진 읽기

모의로 만든 카메라 사진(1920 × 1080, 모듈 하나에 카메라 화소 3개, 모듈 절반만큼의 흐림, 30 dB 잡음,
JPEG 품질 80, 30°까지 기울임, 아무 방향 회전, 어지러운 배경)에서 코드 1,000개 중 992개를 읽었고,
틀리게 읽은 것은 없었습니다. 사진 한 장을 읽는 데 데스크톱 프로세서(AMD Ryzen 5 7500F)에서 중앙값
11.9 ms가 걸렸습니다.

- 45° 기울임에 모듈 절반의 흐림에서는 사진의 32~53%를, 모듈 0.7의 흐림에서는 많아야 58%를
  읽었습니다. 어느 조건에서도 틀리게 읽은 사진은 없었습니다.
- 실제 폰으로 찍은 사진은 아직 재지 않았습니다.
- `cargo run -p nmtcode-sim --release --example measure`로 같은 측정을 다시 할 수 있습니다.

## 설치

[최신 릴리스](https://github.com/needmoretruth/nmtcode/releases/latest)에서 쓰시는 시스템에 맞는 파일을
내려받으십시오.

| 시스템 | 파일 |
|---|---|
| Linux, x86_64 | `nmtcode-<version>-x86_64-unknown-linux-musl.tar.gz` |
| Linux, ARM64 | `nmtcode-<version>-aarch64-unknown-linux-musl.tar.gz` |
| macOS, Apple silicon | `nmtcode-<version>-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `nmtcode-<version>-x86_64-apple-darwin.tar.gz` |
| Windows, x86_64 | `nmtcode-<version>-x86_64-pc-windows-msvc.zip` |

압축을 풀고 `nmtcode`(Windows는 `nmtcode.exe`)를 `PATH`에 두십시오. 릴리스의 `SHA256SUMS`에 모든
파일의 체크섬이 있습니다. macOS 바이너리는 서명하지 않아서, 처음 실행할 때 macOS가 허용할지 묻습니다.

소스에서 빌드하려면 [Rust](https://rustup.rs)를 설치하고 다음을 실행하십시오.

```sh
git clone https://github.com/needmoretruth/nmtcode.git
cd nmtcode
cargo install --path crates/nmtcode-cli --locked
nmtcode --version
```

저장소가 Rust 1.98.1을 고정해 두었고, 첫 빌드 때 `rustup`이 설치합니다.

## CLI 쓰기

```sh
nmtcode make --url https://example.com -o link.png
nmtcode make --text "7시에 북문에서 만나요" -o note.svg
nmtcode make --file report.pdf -o report.png
nmtcode make --profile print --dpi 600 --text "인쇄용 라벨" -o label.png

nmtcode read photo.jpg
nmtcode read report.png --out received/
nmtcode read link.png --json
```

- `make`는 파일 확장자에 따라 PNG나 SVG로 씁니다. `--level 0-3`으로 오류 정정 수준을,
  `--size WxH` · `--max-width` · `--max-height`로 크기를 모듈 수로 정합니다.
- `read`는 PNG나 JPEG를 받습니다. 카메라 사진, 화면 캡처, `make`로 만든 파일 모두 됩니다. 글과
  주소를 출력하고, 주소를 열지 않습니다. 파일은 `--out`을 줄 때만, 있는 파일을 덮어쓰지 않고, 폴더
  경로를 뗀 이름으로 저장합니다.
- 옵션 전부는 `nmtcode make --help`와 `nmtcode read --help`에 있습니다.

## 웹 페이지 쓰기

릴리스의 `nmtcode-web-<version>.zip`은 코드를 만들고, 카메라나 이미지 파일로 읽는 페이지입니다.
모든 일이 브라우저 안에서 일어나고, 입력한 내용과 찍은 사진은 기기 밖으로 나가지 않습니다.

```sh
unzip nmtcode-web-0.0.1.zip
cd nmtcode-web-0.0.1
python3 -m http.server 8000
```

`http://localhost:8000`을 여십시오. 브라우저는 `https://` 페이지와 `localhost`에서만 카메라를
켭니다. 이미지 파일 읽기는 어느 주소에서나 됩니다.

## 라이브러리 쓰기

```rust
use nmtcode::{DecodeOptions, EncodeOptions};
use nmtcode_detect::DetectOptions;
use nmtcode_render::RenderOptions;

let url = "https://github.com/needmoretruth/nmtcode";
let symbol = nmtcode::encode_url(url, &EncodeOptions::default())?;
let png = nmtcode_render::render_png(symbol.grid(), &RenderOptions::default())?;

// Any PNG or JPEG: a camera photo, a screenshot, or the PNG above.
let image = nmtcode_detect::read_image(&png)?;
for found in nmtcode_detect::find(&image, &DetectOptions::default()).found {
    let decoded =
        nmtcode::decode_with_erasures(&found.grid, &found.uncertain, &DecodeOptions::default())?;
    println!("{}", decoded.records[0].text().unwrap_or("(binary record)"));
}
```

전체 프로그램은 [`crates/nmtcode-cli/examples/round_trip.rs`](crates/nmtcode-cli/examples/round_trip.rs)입니다.
크레이트는 아직 crates.io에 없습니다. Git 주소로 의존성을 거십시오.

| 크레이트 | 하는 일 | `no_std` |
|---|---|---|
| `nmtcode` | 기록을 모듈 격자로 만들고 다시 읽기(찾는 쪽이 준 지움 표시 포함) | |
| `nmtcode-core` | 모듈 격자, 형식 단어, 컨테이너, 기록, CRC-32C, 리더 오류 | ✅ |
| `nmtcode-ecc` | GF(2^8) 위의 리드-솔로몬 오류 정정 | ✅ |
| `nmtcode-payload` | 코덱: 그대로, 숫자, 영숫자, 토큰 표, 한글 묶음, brotli | ✅ brotli 없이 |
| `nmtcode-symbol` | 찾기 무늬, 기준 표시, 모듈 배치, 화이트닝 | ✅ |
| `nmtcode-render` | PNG · SVG 출력, 옆에 QR 코드를 붙이거나 빼서 | |
| `nmtcode-detect` | 카메라 사진 · 화면 캡처 · 이미지 파일(PNG, JPEG)에서 코드 찾기 | |
| `nmtcode-cli` | `nmtcode` 명령 | |
| `nmtcode-wasm` | 웹 페이지의 WebAssembly 모듈 | |
| `nmtcode-sim` | 시험과 측정에 쓰는 모의 카메라 사진(게시하지 않음) | |

`no_std` 크레이트는 `wasm32-unknown-unknown`으로도 빌드됩니다. `unsafe`를 쓰는 크레이트는 없습니다.

## 명세

형식은 [`spec/`](spec/01-scope-and-conventions.md)에 영어로 적혀 있고, 지금은 버전 0.2입니다. 비트
하나까지 정해 두어서, 다른 프로그램이 이 코드 없이도 NMT Code를 만들고 읽을 수 있습니다. 명세의
계산 예시와 완성된 시험 코드([부록 A](spec/annex-a-test-vectors.md))는 이 저장소의 시험입니다.

## 검사

```sh
scripts/check.sh
```

서식, clippy, 모든 시험, `wasm32` 빌드, 크레이트 버전이 1.0.0에 닿지 않았는지, 모든 의존성의
라이선스, 저장소에 둘 수 있는 파일인지를 검사합니다.

## 기여

pull request를 받습니다. [기여자 라이선스 동의서](CLA.md)([한국어 설명](CLA.ko.md))에 동의한다는
한 줄이 필요하고, 어디에 적는지는 pull request 양식에 있습니다. 보안 문제는
[SECURITY.md](SECURITY.md)에 적힌 대로 비공개로 알려 주십시오.

## 라이선스

- 코드: [MIT](LICENSE-MIT) 또는 [Apache-2.0](LICENSE-APACHE) 중 고르시면 됩니다.
- 명세(`spec/`): [CC BY 4.0](spec/LICENSE). CC BY 4.0은 특허 권리를 주지 않습니다.

QR Code는 DENSO WAVE INCORPORATED의 등록 상표입니다.
