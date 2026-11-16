# เผยแพร่และอัปเดตผ่าน GitHub

ใช้ public GitHub repository และ GitHub Releases ไม่ต้องให้ผู้ใช้กรอก token
ไม่มีการตรวจหรือดาวน์โหลดเบื้องหลังตอนเปิดโปรแกรม ผู้ใช้คลิกขวาไอคอน > ตรวจสอบอัปเดต
แล้วเลือกติดตั้งเอง ใช้ PowerShell ที่มีใน Windows เฉพาะระหว่างอัปเดต

## เริ่มครั้งแรก

1. สมัครหรือเข้า https://github.com แล้วเปิด https://github.com/new
2. ตั้งชื่อ `pimpit` เลือก **Public** แล้วกด Create repository
3. นำซอร์สขึ้น repository โดยรวม `.github/workflows/release.yml`, `src`, `tools`, `data`,
   Cargo.toml, Cargo.lock, LICENSE และเอกสารด้วย ไม่ต้องอัปโหลด `target`, `dist`, ZIP หรือรุ่นเก่า
4. ที่หน้า repository เลือก Releases > Draft a new release > Choose a tag > สร้าง tag
   `v0.1.6` จาก branch ที่มีซอร์สนี้ แล้ว Publish release
5. รอแท็บ Actions แสดงสำเร็จ แล้วแจกไฟล์ `Pimpit-0.1.6-windows-x64.zip`
   ใน Releases ให้คนอื่นแตกไฟล์และเปิด `Pimpit.exe`

ถ้าใช้ Git ในเครื่องแทนหน้าเว็บ ให้ push ซอร์สก่อนแล้วใช้:

```powershell
git tag v0.1.6
git push origin v0.1.6
```

GitHub Actions จะทดสอบ สร้าง Windows x64 ZIP และ SHA-256 เผยแพร่ Release
พร้อมฝัง `OWNER/REPO` จาก repository ที่ build ลงใน exe ให้ผู้ใช้ทุกคนโดยอัตโนมัติ
ต้องเปิด GitHub Actions และอนุญาต workflow เขียน Releases ตาม `contents: write`
ไม่มีการใส่ token ลงในโปรแกรมหรือไฟล์ตั้งค่าที่แจก

## ออกเวอร์ชันถัดไป

แก้โค้ด เปลี่ยน version ใน Cargo.toml และ Cargo.lock เช่น `0.1.7`
แล้ว push tag `v0.1.7` หรือสร้าง Release/tag ใหม่ผ่านเว็บตามข้างต้น
รอ workflow สำเร็จ ผู้ใช้รุ่น 0.1.4 ขึ้นไปจึงกดตรวจสอบอัปเดตได้
เวอร์ชันก่อน 0.1.4 ต้องดาวน์โหลดและเปิดรุ่นใหม่นี้ครั้งแรกด้วยตนเอง

ยอมรับเฉพาะ stable tag `vX.Y.Z` ที่ใหม่กว่ารุ่นปัจจุบัน และไฟล์
`Pimpit-X.Y.Z-windows-x64.zip` จาก repository ที่กำหนด
ตรวจขนาดและ SHA-256 จาก GitHub API ตรวจ executable x64 และแตกเฉพาะ exe
จึงไม่แตกพาธที่อันตรายจาก ZIP หรือเขียนทับ pimpit.ini

หลังดาวน์โหลดและตรวจเสร็จจึงปิดโปรแกรมอย่างสุภาพ เปลี่ยน exe แบบ File.Replace
สำรองเป็น `Pimpit.exe.previous` แล้วเปิดรุ่นใหม่
ตรวจสถานะเริ่มทำงาน/Raw Input ภายใน 10 วินาที ถ้าไม่พร้อมจะคืน exe เดิมและเปิดใหม่
การตรวจนี้ยืนยันการเริ่มโปรแกรม ไม่รับประกันว่าแอปทุกชนิดแก้คำได้ถูกต้อง
เก็บโปรแกรมในโฟลเดอร์ที่ผู้ใช้มีสิทธิ์เขียน เช่น Documents ไม่ใช่ Program Files

SHA-256 ตรวจความสมบูรณ์ของไฟล์จาก release ของเจ้าของ repo ไม่ใช่ลายเซ็นผู้พัฒนา
เจ้าของบัญชี/repository ที่เผยแพร่จึงต้องเป็นแหล่งที่ผู้ใช้เชื่อถือ

## ใช้รุ่นที่ build เอง

กำหนดก่อน build:

```powershell
$env:PIMPIT_UPDATE_REPO = 'OWNER/pimpit'
.\build.ps1 -Test -BuildOnly
.\tools\Package-Release.ps1
```

หรือแก้ `update_repo=OWNER/pimpit` ใน pimpit.ini แล้วเปิดใหม่
ค่าว่างจะใช้ repo ที่ฝังใน exe รุ่นที่ GitHub build ให้
หากยังไม่มี repo จะมีข้อความแจ้งเมื่อเลือกตรวจสอบอัปเดต ไม่มีแหล่งอัปเดตสมมติ

ทดสอบโดยไม่ดาวน์โหลดหรือปิดโปรแกรมอื่น: `tools/Test-Updater.ps1`
การทดสอบ download/API จริงต้องมี public Release พร้อม asset ที่เผยแพร่แล้ว

อ้างอิง: https://docs.github.com/en/rest/releases/releases
และ https://docs.github.com/en/rest/releases/assets
