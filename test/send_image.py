import smtplib
from email.mime import multipart, text, image

msg = multipart.MIMEMultipart()
msg.attach(text.MIMEText("This is a test email body."))
msg.attach(image.MIMEImage(open("test/assets/rust-1.png", "rb").read()))
msg.attach(text.MIMEText("This is text after the image."))
for i in range(100):
    msg.attach(text.MIMEText(f"This is line {i} of text after the image.\n"))
msg["Subject"] = "Example: Test Image Attachment with Many Lines of Text"
msg["From"] = "user1@example.com"
msg["To"] = "user1@example.com"

# Send via local GreenMail SMTP
with smtplib.SMTP("127.0.0.1", 1025) as server:
    server.send_message(msg)

print("Email sent successfully.")
