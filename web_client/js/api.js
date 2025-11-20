// ============================================================================
// HTTP API Client
// ============================================================================

const API = {
    async request(method, endpoint, body = null) {
        const serverUrl = sessionStorage.getItem('serverUrl') || 'http://localhost:8080';
        const accessToken = sessionStorage.getItem('accessToken');

        const headers = {
            'Content-Type': 'application/json',
        };

        if (accessToken) {
            headers['Authorization'] = `Bearer ${accessToken}`;
        }

        const options = {
            method,
            headers,
        };

        if (body) {
            options.body = JSON.stringify(body);
        }

        try {
            const response = await fetch(`${serverUrl}${endpoint}`, options);

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(errorText || `HTTP ${response.status}`);
            }

            const contentType = response.headers.get('content-type');
            if (contentType && contentType.includes('application/json')) {
                return await response.json();
            }

            return null;
        } catch (error) {
            throw new Error(`API Error: ${error.message}`);
        }
    },

    async login(username, password) {
        const data = await this.request('POST', '/api/auth/login', {
            username,
            password
        });

        // Store tokens
        sessionStorage.setItem('accessToken', data.access_token);
        sessionStorage.setItem('refreshToken', data.refresh_token);
        sessionStorage.setItem('username', data.username);
        sessionStorage.setItem('userId', data.user_id);

        return data;
    },

    async register(username, password, displayName) {
        const data = await this.request('POST', '/api/auth/register', {
            username,
            password,
            display_name: displayName || username
        });

        // Store tokens
        sessionStorage.setItem('accessToken', data.access_token);
        sessionStorage.setItem('refreshToken', data.refresh_token);
        sessionStorage.setItem('username', data.username);
        sessionStorage.setItem('userId', data.user_id);

        return data;
    }
};

// Helper function for table listing (used in lobby)
async function fetchTables() {
    return await API.request('GET', '/api/tables');
}
