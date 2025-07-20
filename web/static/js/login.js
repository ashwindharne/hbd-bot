document.addEventListener("DOMContentLoaded", function () {
  const form = document.getElementById("loginForm");
  const phoneInput = document.getElementById("phone");
  const codeInput = document.getElementById("code");
  const verificationSection = document.getElementById("verificationSection");
  const smsMessage = document.getElementById("smsMessage");
  const submitBtn = document.getElementById("submitBtn");
  const resultDiv = document.getElementById("result");
  const phoneError = document.getElementById("phoneError");
  const otpError = document.getElementById("otpError");
  const formattedPhoneNumber = document.getElementById("formattedPhoneNumber");
  const resendLink = document.getElementById("resendLink");

  let countdown;
  let isOtpMode = false;
  let currentPhone = "";

  // Handle OTP input formatting - only allow digits
  codeInput.addEventListener("input", function (e) {
    let value = e.target.value.replace(/\D/g, "");

    // Limit to 6 digits
    if (value.length > 6) {
      value = value.substring(0, 6);
    }

    e.target.value = value;

    // Clear OTP error when user starts typing
    otpError.classList.add("hidden");
    codeInput.classList.remove("error");

    // Update button state based on OTP validation
    validateOtpAndUpdateButton();
  });

  // Handle phone input - numbers only, max 10 digits
  phoneInput.addEventListener("input", function (e) {
    let value = e.target.value.replace(/\D/g, "");

    // Limit to 10 digits
    if (value.length > 10) {
      value = value.substring(0, 10);
    }

    e.target.value = value;

    // Clear error message when user starts typing
    phoneError.classList.add("hidden");
    resultDiv.innerHTML = "";

    // Reset OTP mode only when phone number actually changes
    if (isOtpMode && value !== currentPhone) {
      resetToPhoneMode();
    }

    // Update button state based on phone validation
    validatePhoneAndUpdateButton();
  });

  // Handle form submission
  form.addEventListener("submit", function (e) {
    e.preventDefault();

    if (!isOtpMode) {
      // Send OTP
      sendOtp();
    } else {
      // Verify OTP
      verifyOtp();
    }
  });

  // Handle resend link
  resendLink.addEventListener("click", function (e) {
    e.preventDefault();
    sendOtp();
  });

  function sendOtp() {
    const phone = phoneInput.value.trim();

    // Client-side validation
    if (!phone) {
      showError("Please enter a phone number");
      return;
    }

    // Disable form during OTP request
    submitBtn.disabled = true;
    submitBtn.textContent = "Sending...";

    fetch("/login", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ phone: phone }),
    })
      .then((response) => response.json())
      .then((data) => {
        if (data.success) {
          currentPhone = phone;
          showOtpMode(data.validity_seconds, phone);
        } else {
          // Re-enable form on error
          submitBtn.disabled = false;
          submitBtn.textContent = "I agree";
          showError(data.message);
        }
      })
      .catch((error) => {
        // Re-enable form on error
        submitBtn.disabled = false;
        submitBtn.textContent = "I agree";
        showError("Error sending OTP");
        console.error("Error:", error);
      });
  }

  function verifyOtp() {
    const phone = phoneInput.value.trim();
    const code = codeInput.value.trim();

    if (!code) {
      showOtpError("Please enter the verification code");
      return;
    }

    submitBtn.disabled = true;
    submitBtn.textContent = "Verifying...";

    fetch("/verify-otp", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ phone: phone, code: code }),
    })
      .then((response) => {
        // Check for HX-Redirect header
        const hxRedirect = response.headers.get("HX-Redirect");
        if (hxRedirect) {
          window.location.href = hxRedirect;
          return;
        }

        return response.json();
      })
      .then((data) => {
        if (data && !data.success) {
          submitBtn.disabled = false;
          submitBtn.textContent = "I agree";
          showOtpError(data.message);
        }
      })
      .catch((error) => {
        submitBtn.disabled = false;
        submitBtn.textContent = "I agree";
        showOtpError("Error verifying OTP");
        console.error("Error:", error);
      });
  }

  function showOtpMode(validitySeconds, phone) {
    isOtpMode = true;
    verificationSection.classList.remove("hidden");
    smsMessage.classList.remove("hidden");

    // Format phone number as +1 323-123-1234
    const formatted = `+1 ${phone.substring(0, 3)}-${phone.substring(3, 6)}-${phone.substring(6)}`;
    formattedPhoneNumber.textContent = formatted;

    submitBtn.textContent = "I agree";
    codeInput.focus();

    // Validate OTP and update button state
    validateOtpAndUpdateButton();

    startCountdown(validitySeconds);
  }

  function resetToPhoneMode() {
    isOtpMode = false;
    verificationSection.classList.add("hidden");
    smsMessage.classList.add("hidden");
    codeInput.value = "";
    resultDiv.innerHTML = "";
    submitBtn.textContent = "Login";
    validatePhoneAndUpdateButton();

    // Reset resend section
    const resendText = document.getElementById("resendText");
    resendText.classList.add("hidden");
    resendLink.classList.add("hidden");

    // Clear OTP error
    otpError.classList.add("hidden");
    codeInput.classList.remove("error");

    if (countdown) {
      clearInterval(countdown);
    }
  }

  function startCountdown(seconds) {
    let timeLeft = seconds;
    const resendText = document.getElementById("resendText");
    const resendTimer = document.getElementById("resendTimer");

    function updateTimer() {
      if (timeLeft > 0) {
        resendTimer.textContent = timeLeft;
        timeLeft--;
      } else {
        clearInterval(countdown);

        // Show resend link when timer expires
        resendText.classList.add("hidden");
        resendLink.classList.remove("hidden");
      }
    }

    // Show countdown text initially
    resendText.classList.remove("hidden");
    resendLink.classList.add("hidden");

    updateTimer();
    countdown = setInterval(updateTimer, 1000);
  }

  function showError(message) {
    phoneError.textContent = message;
    phoneError.classList.remove("hidden");
    resultDiv.innerHTML = "";
  }

  function showOtpError(message) {
    otpError.textContent = message;
    otpError.classList.remove("hidden");
    codeInput.classList.add("error");
    resultDiv.innerHTML = "";

    // Disable button when error is shown
    submitBtn.disabled = true;
  }

  function showSuccess(message) {
    phoneError.classList.add("hidden");
    resultDiv.innerHTML = `<div class="success-message">${message}</div>`;
  }

  function validatePhoneAndUpdateButton() {
    const phone = phoneInput.value.trim();

    if (!isOtpMode) {
      if (phone.length === 10) {
        submitBtn.disabled = false;
      } else {
        submitBtn.disabled = true;
      }
    }
  }

  function validateOtpAndUpdateButton() {
    if (isOtpMode) {
      const code = codeInput.value.trim();
      const hasError = codeInput.classList.contains("error");

      // Enable button only if code is 6 digits and no error state
      if (code.length === 6 && !hasError) {
        submitBtn.disabled = false;
      } else {
        submitBtn.disabled = true;
      }
    }
  }
});
