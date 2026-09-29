# Reporting a security problem

*[한국어는 아래에 있습니다.](#보안-문제-신고)*

**Report it privately first**, not as a public issue.

- **GitHub** — the "Report a vulnerability" button on the Security tab of this repository. Only the
  maintainer sees the report.
- **Email** — **nmts@nmts.me**.

Write in English or in Korean. Both are read.

## What counts

NMT Code reads images and symbols that anyone can make. These are security problems:

- an image, a symbol or a file that makes a reader crash, hang, or use memory without bound;
- one symbol that two readers following the specification present as different content;
- a URL or a file name that a reader presents in a way that hides where it leads or where the file
  is saved;
- a rule of the [specification](spec/01-scope-and-conventions.md) that allows one of the above.

## What to put in it

What you did, what happened, and the image or bytes that show it. If the report needs a file, say
so and you will be told where to put it.

## What is not promised

No bounty, no payment, and no fixed reply time. Reports are read.

---

# 보안 문제 신고

**먼저 비공개로 알려 주십시오.** 공개 이슈로 올리지 말아 주십시오.

- **GitHub** — 이 저장소 Security 탭의 「Report a vulnerability」 버튼. 신고는 관리자만 봅니다.
- **이메일** — **nmts@nmts.me**.

영어와 한국어 둘 다 읽습니다.

## 무엇이 보안 문제인가

NMT Code는 누구나 만들 수 있는 이미지와 코드를 읽습니다. 다음이 보안 문제입니다.

- 리더를 멈추게 하거나, 끝나지 않게 하거나, 메모리를 한없이 쓰게 하는 이미지 · 코드 · 파일
- 명세를 따르는 두 리더가 서로 다른 내용으로 보여 주는 코드 하나
- 링크가 실제로 어디로 가는지, 파일이 어디에 저장되는지를 가리게 보여 주는 URL이나 파일 이름
- 위의 일 가운데 하나를 허용하는 [명세](spec/01-scope-and-conventions.md)의 규칙

## 무엇을 적어 주시면 되는가

무엇을 하셨고, 무엇이 일어났는지, 그리고 그것을 보여 주는 이미지나 바이트. 파일이 필요한 신고라면
그렇다고만 적어 주십시오. 어디에 두시면 되는지 알려 드립니다.

## 약속하지 않는 것

포상금도, 대가 지급도, 정해진 답장 기한도 없습니다. 신고는 읽습니다.
