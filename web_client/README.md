# Private Poker - Web Client

A modern web-based client for Private Poker that runs in any web browser. Similar functionality to the TUI client but with a visual interface.

## Features

- 🎨 **Beautiful UI**: Modern, responsive design with poker table layout
- 🃏 **Visual Cards**: Colorful playing cards with suit symbols
- ⚡ **Real-time Updates**: WebSocket connection for live game updates
- 🎮 **Full Game Controls**: Join, Fold, Check, Call, Raise, All-In
- 📊 **Game Log**: Track all actions and events
- 👥 **Multiplayer**: See other players around the table
- 🔒 **Secure**: JWT authentication

## Project Structure

```
web_client/
├── index.html          # Login/Register page
├── lobby.html          # Table selection
├── game.html           # Poker table game view
├── css/
│   ├── main.css        # Main styles
│   └── cards.css       # Card rendering styles
└── js/
    ├── api.js          # HTTP API client
    ├── auth.js         # Authentication logic
    ├── websocket.js    # WebSocket manager
    └── game.js         # Game UI and logic
```

## How to Use

### 1. Start the Server

Make sure the poker server is running:

```bash
# From project root
cargo run --bin pp_server --release
```

The server should be running on `http://localhost:8080` (or your configured port).

### 2. Open the Web Client

Simply open `index.html` in your web browser:

```bash
# Using Python's built-in server (recommended)
cd web_client
python3 -m http.server 8000

# Then open in browser:
# http://localhost:8000
```

Or directly open `index.html` in your browser (file:// protocol).

### 3. Login or Register

1. Enter the server URL (default: `http://localhost:8080`)
2. Enter your username and password
3. Click "Login" to login or "Register" to create a new account

**Password Requirements:**
- At least 8 characters
- One uppercase letter (A-Z)
- One lowercase letter (a-z)
- One number (0-9)

### 4. Select a Table

After logging in, you'll see the lobby with available tables:
- View player count and blinds
- Click "Join Table" to enter a game

### 5. Play Poker!

Once at the table:
1. Enter your buy-in amount and click "Join Table"
2. Wait for your turn
3. Use action buttons:
   - **Fold**: Give up your hand
   - **Check**: Pass without betting
   - **Call**: Match the current bet
   - **Raise**: Increase the bet (enter amount)
   - **All-In**: Bet all your chips

## UI Components

### Login Page
- Server URL configuration
- Username/password input
- Login and Register buttons
- Password requirements info

### Lobby
- List of available tables
- Player count and blinds display
- Refresh button
- Auto-refresh every 5 seconds

### Game Table
- **Poker Table**: Circular green felt with community cards
- **Pot Display**: Current pot amount in center
- **Blinds Info**: Small/big blind amounts
- **Player Seats**: Shows players around the table with:
  - Player name
  - Chip count
  - Current state (Waiting, Folded, etc.)
- **Your Hand**: Your 2 hole cards displayed at bottom
- **Action Panel**: Buttons for game actions
- **Game Log**: Scrollable history of all actions

## Technical Details

### Technologies Used
- **HTML5**: Structure
- **CSS3**: Styling with animations
- **Vanilla JavaScript**: No frameworks needed
- **WebSocket**: Real-time communication
- **Fetch API**: HTTP requests

### API Endpoints Used
- `POST /api/auth/register` - Create new account
- `POST /api/auth/login` - Authenticate user
- `GET /api/tables` - List available tables
- `WebSocket /ws/:table_id?token=<jwt>` - Game connection

### WebSocket Messages

**Sent to Server:**
```javascript
// Join table
{ "type": "join", "buy_in": 1000 }

// Leave table
{ "type": "leave" }

// Game action
{ "type": "action", "action": { "type": "fold" } }
{ "type": "action", "action": { "type": "call" } }
{ "type": "action", "action": { "type": "raise", "amount": 100 } }
```

**Received from Server:**
```javascript
// Game view update
{
  "blinds": { "small": 10, "big": 20 },
  "pot": { "size": 150 },
  "board": [{ "rank": "A", "suit": "spades" }, ...],
  "players": [
    {
      "user": { "name": "alice", "money": 950 },
      "state": "wait",
      "cards": [...]
    },
    ...
  ]
}
```

## Browser Compatibility

Tested and working on:
- ✅ Chrome/Edge (latest)
- ✅ Firefox (latest)
- ✅ Safari (latest)

Requires:
- Modern browser with WebSocket support
- JavaScript enabled
- No special plugins needed

## Development

To modify the web client:

1. **Styles**: Edit CSS files in `css/` directory
2. **Logic**: Edit JavaScript files in `js/` directory
3. **Layout**: Edit HTML files in root directory

All changes take effect immediately (just refresh browser).

## Troubleshooting

### Can't Connect to Server
- Verify server is running on the specified URL
- Check server port matches (default: 8080)
- Try `http://localhost:8080` instead of `127.0.0.1`

### WebSocket Connection Fails
- Make sure server URL uses `http://` (will auto-convert to `ws://`)
- Check browser console for error messages
- Verify JWT token is valid (try logging in again)

### Cards Not Displaying
- Check browser console for JavaScript errors
- Ensure all CSS files are loaded
- Clear browser cache and refresh

### Actions Not Working
- Verify you've joined the table (clicked "Join Table" button)
- Check it's your turn (player seat should be highlighted)
- Look at game log for error messages

## Features Compared to TUI Client

| Feature | TUI Client | Web Client |
|---------|-----------|------------|
| Visual Cards | Colored text | Full graphical cards |
| Player Display | List view | Circular table layout |
| Real-time Updates | ✅ | ✅ |
| Game Actions | ✅ | ✅ |
| Login/Register | ✅ | ✅ |
| Table Selection | ✅ | ✅ |
| Platform | Terminal | Any browser |
| Installation | Cargo build | None (just open HTML) |

## Screenshots

_(Web client is fully styled and ready to use!)_

## Future Enhancements

Potential improvements:
- Sound effects for game events
- Animations for card dealing
- Chat feature
- Player avatars
- Mobile-responsive layout
- Dark/light theme toggle
- Statistics dashboard

## License

Apache License 2.0 - Same as the main project

---

**Built by**: Saman Sohani
**Project**: Private Poker Web Client
**Version**: 1.0.0
**Date**: November 2025
