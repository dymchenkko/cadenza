# Cadenza Web UI

Cadenza now includes a beautiful web interface for managing your Solana development environment!

## 🚀 Quick Start

1. **Start the web server:**
   ```bash
   cargo run -- web
   ```

   Or specify a custom port:
   ```bash
   cargo run -- web --port 3000
   ```

2. **Open your browser:**
   Navigate to `http://localhost:8080` (or your custom port)

3. **Start using the UI:**
   - Click "Start Validator" to launch your local Solana environment
   - Create and manage snapshots with the visual interface
   - View your configuration in a formatted display

## ✨ Features

### Validator Management
- **Start/Stop Validator**: Control your local Solana validator with one click
- **Status Monitoring**: Real-time status indicator showing if the validator is running
- **RPC URL Display**: See your validator's RPC endpoint at a glance

### Snapshot Management
- **Create Snapshots**: Save your current ledger state with a custom name
- **Load Snapshots**: Restore any saved snapshot instantly
- **Snapshot List**: View all available snapshots in a clean list
- **Quick Actions**: Select and load snapshots with a single click

### Configuration Viewer
- **Live Config Display**: View your current `cadenza-config.json` in a formatted view
- **Auto-refresh**: Refresh button to reload configuration

## 🎨 UI Features

- **Modern Design**: Beautiful gradient background and clean card-based layout
- **Responsive**: Works on desktop and tablet devices
- **Real-time Updates**: Status auto-refreshes every 5 seconds
- **Error Handling**: Clear error messages for any issues
- **Loading States**: Visual feedback during operations

## 🔧 Technical Details

The web UI runs on Axum (async Rust web framework) and serves:
- Static HTML/CSS/JS frontend from the `web/` directory
- RESTful API endpoints at `/api/*`
- CORS enabled for development

### API Endpoints

- `GET /api/status` - Get validator status
- `POST /api/start` - Start the validator
- `POST /api/stop` - Stop the validator
- `GET /api/snapshots` - List all snapshots
- `POST /api/snapshots/create` - Create a snapshot
- `POST /api/snapshots/load` - Load a snapshot
- `GET /api/config` - Get current configuration

## 💡 Tips

1. **Keep the web UI running**: The web server runs independently - you can keep it open while working
2. **Validator must be stopped**: Before creating/loading snapshots, make sure the validator is stopped
3. **Config path**: Use `--config-path` to specify a different config file:
   ```bash
   cargo run -- web --config-path my-config.json
   ```

## 🐛 Troubleshooting

**Port already in use:**
```bash
# Use a different port
cargo run -- web --port 3000
```

**Validator won't start:**
- Check that `cadenza-config.json` exists
- Ensure Solana CLI tools are installed
- Check that required ports (8899, 9001, 9900) are available

**Web UI not loading:**
- Make sure the `web/` directory exists with `index.html`
- Check browser console for errors
- Verify the server started successfully (check terminal output)

---

**Enjoy your visual Solana development environment! 🎹**

