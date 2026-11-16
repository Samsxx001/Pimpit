Pimpit

โปรแกรมแก้ข้อความที่พิมพ์ผิดแป้นไทย/อังกฤษ สำหรับ Windows 10/11 x64 เขียนด้วย Rust

- แก้อัตโนมัติขณะพิมพ์ ไม่ต้องกด Space
- Shift+Backspace แปลงคำล่าสุด และ Ctrl+Alt+Z ย้อนการแปลง
- พัก Auto เมื่อหน้าต่างเกมอยู่ด้านหน้า แล้วคืนค่าที่เลือกไว้เมื่อสลับออก
- ตรวจสอบและติดตั้งอัปเดตผ่านเมนูคลิกขวาที่ไอคอน เก็บค่าตั้งค่าเดิม
- การแก้คำทำงานออฟไลน์ ไม่มีการส่งคำที่พิมพ์ไปเซิร์ฟเวอร์

ดาวน์โหลด

เปิด Releases (https://github.com/Samsxx001/Pimpit/releases/latest) ดาวน์โหลดไฟล์ Pimpit-VERSION-windows-x64.zip
แตกไฟล์ลงโฟลเดอร์ที่เขียนได้ แล้วเปิด Pimpit.exe
รุ่นก่อน 0.1.4 ต้องดาวน์โหลดรุ่นใหม่ด้วยตนเองครั้งแรก

โปรแกรมนี้เป็นโครงการอิสระ ไม่ใช่ RightLang ตัวเดิม

ดู คู่มือภาษาไทย (README-TH.txt), ระบบอัปเดตและการเผยแพร่ (GITHUB-TH.md)
และ ผลตรวจสอบและข้อจำกัด (VALIDATION-TH.md)

Build

ใช้ Rust stable และ Windows SDK/MSVC linker แล้วรัน cargo test --locked และ
cargo build --release --locked หรือใช้ build.ps1 สำหรับ GNU toolchain ตามคู่มือ
สร้างแพ็กเกจด้วย tools/Package-Release.ps1

สิทธิ์ซอร์ส MIT พจนานุกรมมีสิทธิ์แยกตาม data/SOURCES.md
