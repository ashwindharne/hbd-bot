# HBD-Bot

This is a bot that sends a daily SMS reminder to users about their friends' birthdays.

It's a single Rust monorepo that produces two binaries: `web`, and `sweeper`.
- `web` is an axum web server that serves a web interface for managing users' friends' birthdays.
- `sweeper` is a process that runs every hour to send SMS reminders to users about upcoming birthdays.

## OTP Authentication Flow

The web application uses SMS-based OTP (One-Time Password) authentication for user login.

### Frontend Flow
1. **Phone Input**: User enters 10-digit phone number (numbers only, max length 10)
2. **Login Button**: Enabled when phone number is entered, starts as "Login"
3. **OTP Request**: Clicking "Login" sends POST to `/login` endpoint
4. **OTP Mode**: On success, shows:
   - OTP input field (6 digits max)
   - SMS message: "We sent +1 XXX-XXX-XXXX a code via SMS"
   - Terms and conditions text
   - Button changes to "I Agree"
   - Resend countdown timer
5. **OTP Verification**: Button enabled when 6-digit code is entered
6. **Success**: Server sends HX-Redirect header to redirect user

### Backend Endpoints
- `POST /login`: Accepts `{phone: string}`, returns `{success: bool, message: string, validity_seconds: number}`
- `POST /verify-otp`: Accepts `{phone: string, code: string}`, returns `{success: bool, message: string}` with optional `HX-Redirect` header

### UI Features
- US flag display with +1 country code
- Phone number formatting in SMS message (+1 XXX-XXX-XXXX)
- Resend timer countdown with "Didn't receive a code? Resend it in XXs" 
- When timer expires: "Didn't receive a code? Resend it" (with link)
- Separate error displays for phone and OTP fields
- Button states: disabled/enabled based on input validation

### Files
- `web/templates/login.html`: Login page template
- `web/static/css/login.css`: Login page styles
- `web/static/js/login.js`: Login page JavaScript
- `web/src/main.rs`: Server-side endpoints (lines 178-262)
