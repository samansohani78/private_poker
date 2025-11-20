// ============================================================================
// WebSocket Connection Manager
// ============================================================================

class WebSocketManager {
    constructor() {
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        this.onGameUpdate = null;
    }

    connect(tableId) {
        const serverUrl = sessionStorage.getItem('serverUrl') || 'http://localhost:8080';
        const accessToken = sessionStorage.getItem('accessToken');

        // Convert http to ws
        const wsUrl = serverUrl.replace('http://', 'ws://').replace('https://', 'wss://');
        const url = `${wsUrl}/ws/${tableId}?token=${accessToken}`;

        console.log('Connecting to WebSocket:', url);

        this.ws = new WebSocket(url);

        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.reconnectAttempts = 0;
            this.updateStatus('Connected to table', 'success');
        };

        this.ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                console.log('Received game update:', data);

                if (this.onGameUpdate) {
                    this.onGameUpdate(data);
                }
            } catch (error) {
                console.error('Failed to parse WebSocket message:', error);
            }
        };

        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.updateStatus('Connection error', 'error');
        };

        this.ws.onclose = () => {
            console.log('WebSocket closed');
            this.updateStatus('Disconnected from table', 'error');

            // Attempt to reconnect
            if (this.reconnectAttempts < this.maxReconnectAttempts) {
                this.reconnectAttempts++;
                console.log(`Reconnecting... Attempt ${this.reconnectAttempts}`);
                setTimeout(() => this.connect(tableId), 2000);
            } else {
                this.updateStatus('Failed to connect. Please refresh the page.', 'error');
            }
        };
    }

    send(command) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(command));
            console.log('Sent command:', command);
        } else {
            console.error('WebSocket is not connected');
            this.updateStatus('Not connected to server', 'error');
        }
    }

    sendJoin(buyIn) {
        this.send({
            type: 'join',
            buy_in: buyIn
        });
    }

    sendLeave() {
        this.send({
            type: 'leave'
        });
    }

    sendAction(actionType, amount = null) {
        const action = {
            type: actionType.toLowerCase()
        };

        if (amount !== null) {
            action.amount = amount;
        }

        this.send({
            type: 'action',
            action: action
        });
    }

    updateStatus(message, type = 'info') {
        const statusDiv = document.getElementById('status-message');
        if (statusDiv) {
            statusDiv.textContent = message;
            statusDiv.style.display = 'block';
            statusDiv.className = `status-message ${type}`;

            if (type === 'success') {
                setTimeout(() => {
                    statusDiv.style.display = 'none';
                }, 3000);
            }
        }
    }

    disconnect() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }
}

// Global instance
const wsManager = new WebSocketManager();
