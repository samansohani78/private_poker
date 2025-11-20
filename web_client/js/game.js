// ============================================================================
// Game UI and Logic
// ============================================================================

// Check authentication
const serverUrl = sessionStorage.getItem('serverUrl');
const accessToken = sessionStorage.getItem('accessToken');
const username = sessionStorage.getItem('username');
const tableId = sessionStorage.getItem('tableId');

if (!serverUrl || !accessToken || !tableId) {
    window.location.href = 'index.html';
}

// Display username
document.getElementById('username-display').textContent = `Player: ${username}`;

// Current game state
let currentGameState = null;
let hasJoinedTable = false;

// ============================================================================
// Initialization
// ============================================================================

// Leave table button
document.getElementById('leave-table-btn').addEventListener('click', () => {
    wsManager.sendLeave();
    wsManager.disconnect();
    window.location.href = 'lobby.html';
});

// Join button
document.getElementById('join-btn').addEventListener('click', () => {
    const buyIn = parseInt(document.getElementById('buy-in-amount').value);
    if (buyIn > 0) {
        wsManager.sendJoin(buyIn);
        document.getElementById('join-controls').style.display = 'none';
        hasJoinedTable = true;
        addLogEntry(`Joining table with $${buyIn}...`);
    }
});

// Action buttons
document.getElementById('fold-btn').addEventListener('click', () => {
    wsManager.sendAction('fold');
    addLogEntry('You folded');
});

document.getElementById('check-btn').addEventListener('click', () => {
    wsManager.sendAction('check');
    addLogEntry('You checked');
});

document.getElementById('call-btn').addEventListener('click', () => {
    wsManager.sendAction('call');
    addLogEntry('You called');
});

document.getElementById('raise-btn').addEventListener('click', () => {
    const amount = parseInt(document.getElementById('raise-amount').value);
    if (amount > 0) {
        wsManager.sendAction('raise', amount);
        addLogEntry(`You raised $${amount}`);
    }
});

document.getElementById('allin-btn').addEventListener('click', () => {
    wsManager.sendAction('all_in');
    addLogEntry('You went all-in!');
});

// ============================================================================
// WebSocket Connection
// ============================================================================

wsManager.onGameUpdate = (gameView) => {
    currentGameState = gameView;
    updateUI(gameView);
};

wsManager.connect(tableId);

// ============================================================================
// UI Update Functions
// ============================================================================

function updateUI(gameView) {
    updateBlinds(gameView.blinds);
    updatePot(gameView.pot);
    updateBoard(gameView.board);
    updatePlayers(gameView.players);
    updateYourHand(gameView.players);
    updateActionButtons(gameView.players);
}

function updateBlinds(blinds) {
    document.getElementById('blinds-info').textContent = `$${blinds.small}/$${blinds.big}`;
}

function updatePot(pot) {
    document.getElementById('pot-amount').textContent = `$${pot.size}`;
}

function updateBoard(board) {
    const boardCards = document.getElementById('board-cards');
    boardCards.innerHTML = '';

    if (board.length === 0) {
        boardCards.innerHTML = '<div style="color: #aaa;">No cards yet</div>';
        return;
    }

    board.forEach(card => {
        boardCards.appendChild(createCard(card));
    });
}

function updatePlayers(players) {
    const playersContainer = document.getElementById('players-container');
    playersContainer.innerHTML = '';

    // Arrange players in a circle
    const angleStep = (2 * Math.PI) / Math.max(players.length, 6);
    const radius = 200;

    players.forEach((player, index) => {
        const angle = angleStep * index - Math.PI / 2;
        const x = 50 + (radius * Math.cos(angle)) / 2;
        const y = 50 + (radius * Math.sin(angle)) / 2;

        const playerSeat = createPlayerSeat(player, x, y);
        playersContainer.appendChild(playerSeat);
    });
}

function createPlayerSeat(player, x, y) {
    const seat = document.createElement('div');
    seat.className = 'player-seat';
    seat.style.left = `${x}%`;
    seat.style.top = `${y}%`;
    seat.style.transform = 'translate(-50%, -50%)';

    // Check if this is the current user
    const isYou = player.user.name === username;

    // Check player state
    const state = player.state;
    if (state === 'wait' || state === 'raise' || state === 'call' || state === 'check') {
        seat.classList.add('active');
    }

    seat.innerHTML = `
        <div class="player-name">${player.user.name}${isYou ? ' (You)' : ''}</div>
        <div class="player-chips">$${player.user.money}</div>
        <div class="player-state">${formatState(state)}</div>
    `;

    return seat;
}

function formatState(state) {
    const stateMap = {
        'wait': 'Waiting',
        'fold': 'Folded',
        'call': 'Called',
        'check': 'Checked',
        'raise': 'Raised',
        'all_in': 'All-In'
    };
    return stateMap[state] || state;
}

function updateYourHand(players) {
    const yourPlayer = players.find(p => p.user.name === username);

    if (!yourPlayer) {
        document.getElementById('your-chips').textContent = 'Not in game';
        return;
    }

    // Update chips
    document.getElementById('your-chips').textContent = `Chips: $${yourPlayer.user.money}`;

    // Update cards
    const yourCards = document.getElementById('your-cards');
    yourCards.innerHTML = '';

    if (yourPlayer.cards && yourPlayer.cards.length > 0) {
        yourPlayer.cards.forEach(card => {
            yourCards.appendChild(createCard(card));
        });
    } else {
        // Show card backs
        yourCards.innerHTML = `
            <div class="card-back"></div>
            <div class="card-back"></div>
        `;
    }
}

function updateActionButtons(players) {
    const yourPlayer = players.find(p => p.user.name === username);

    if (!yourPlayer) {
        document.getElementById('game-controls').style.display = 'none';
        return;
    }

    // Show game controls if joined
    if (hasJoinedTable) {
        document.getElementById('game-controls').style.display = 'flex';
    }

    // You can add more logic here to enable/disable buttons based on game state
}

// ============================================================================
// Card Rendering
// ============================================================================

function createCard(card) {
    const cardDiv = document.createElement('div');
    cardDiv.className = `card ${card.suit.toLowerCase()} dealing`;

    const suitSymbol = getSuitSymbol(card.suit);
    const rankText = card.rank;

    cardDiv.innerHTML = `
        <div class="card-top">
            <span class="card-rank">${rankText}</span>
            <span class="card-suit">${suitSymbol}</span>
        </div>
        <div class="card-center">${suitSymbol}</div>
        <div class="card-bottom">
            <span class="card-rank">${rankText}</span>
            <span class="card-suit">${suitSymbol}</span>
        </div>
    `;

    return cardDiv;
}

function getSuitSymbol(suit) {
    const suitMap = {
        'hearts': '♥',
        'diamonds': '♦',
        'clubs': '♣',
        'spades': '♠',
        'Hearts': '♥',
        'Diamonds': '♦',
        'Clubs': '♣',
        'Spades': '♠'
    };
    return suitMap[suit] || suit;
}

// ============================================================================
// Game Log
// ============================================================================

function addLogEntry(message) {
    const logContent = document.getElementById('log-content');
    const entry = document.createElement('div');
    entry.className = 'log-entry';
    entry.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;

    logContent.appendChild(entry);

    // Auto-scroll to bottom
    logContent.scrollTop = logContent.scrollHeight;

    // Keep only last 50 entries
    while (logContent.children.length > 50) {
        logContent.removeChild(logContent.firstChild);
    }
}

// Initial log entry
addLogEntry('Connected to table. Waiting for game updates...');
