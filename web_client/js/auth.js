// ============================================================================
// Authentication Handler
// ============================================================================

document.getElementById('login-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    await handleLogin();
});

document.getElementById('register-btn').addEventListener('click', async () => {
    await handleRegister();
});

async function handleLogin() {
    const serverUrl = document.getElementById('server-url').value.trim();
    const username = document.getElementById('username').value.trim();
    const password = document.getElementById('password').value;

    if (!serverUrl || !username || !password) {
        showError('Please fill in all fields');
        return;
    }

    // Store server URL
    sessionStorage.setItem('serverUrl', serverUrl);

    try {
        showLoading('Logging in...');

        await API.login(username, password);

        // Redirect to lobby
        window.location.href = 'lobby.html';
    } catch (error) {
        showError('Login failed: ' + error.message);
    }
}

async function handleRegister() {
    const serverUrl = document.getElementById('server-url').value.trim();
    const username = document.getElementById('username').value.trim();
    const password = document.getElementById('password').value;

    if (!serverUrl || !username || !password) {
        showError('Please fill in all fields');
        return;
    }

    // Validate password strength
    if (!validatePassword(password)) {
        showError('Password does not meet requirements');
        return;
    }

    // Store server URL
    sessionStorage.setItem('serverUrl', serverUrl);

    try {
        showLoading('Registering...');

        await API.register(username, password, username);

        // Redirect to lobby
        window.location.href = 'lobby.html';
    } catch (error) {
        showError('Registration failed: ' + error.message);
    }
}

function validatePassword(password) {
    // At least 8 characters
    if (password.length < 8) return false;

    // At least one uppercase
    if (!/[A-Z]/.test(password)) return false;

    // At least one lowercase
    if (!/[a-z]/.test(password)) return false;

    // At least one number
    if (!/[0-9]/.test(password)) return false;

    return true;
}

function showError(message) {
    const errorDiv = document.getElementById('error-message');
    errorDiv.textContent = message;
    errorDiv.style.display = 'block';

    // Hide loading buttons
    document.getElementById('login-btn').disabled = false;
    document.getElementById('register-btn').disabled = false;

    setTimeout(() => {
        errorDiv.style.display = 'none';
    }, 5000);
}

function showLoading(message) {
    const errorDiv = document.getElementById('error-message');
    errorDiv.textContent = message;
    errorDiv.style.display = 'none';

    // Disable buttons during loading
    document.getElementById('login-btn').disabled = true;
    document.getElementById('register-btn').disabled = true;
}
