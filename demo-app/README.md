# SwapTrade Demo App

A React-based demonstration application for the SwapTrade SDK, showcasing atomic swap functionality on Stellar Soroban.

## Features

- **Interactive UI**: Modern React interface with TailwindCSS styling
- **Full Swap Lifecycle**: Create, fund, accept, and cancel atomic swaps
- **Real-time Status**: Visual feedback for swap states and transactions
- **Demo Mode**: Generate demo keypairs for testing without wallet setup
- **Responsive Design**: Works on desktop and mobile devices

## Prerequisites

- Node.js 18+ and npm/yarn
- Soroban CLI installed
- Localnet running (or configured testnet/mainnet)
- Deployed atomic swap contract

## Installation

```bash
cd demo-app
npm install
```

## Configuration

1. Copy the example environment file:
```bash
cp .env.example .env
```

2. Edit `.env` with your configuration:
```env
VITE_SWAP_CONTRACT_ID=your_contract_id_here
VITE_RPC_URL=http://localhost:8000/soroban/rpc
VITE_NETWORK_PASSPHRASE=Standalone Network ; February 2017
```

## Running the Demo

### Development Mode

```bash
npm run dev
```

The app will be available at `http://localhost:3000`

### Production Build

```bash
npm run build
npm run preview
```

## Running E2E Tests

```bash
# Run tests
npm run test:e2e

# Run tests with UI
npm run test:e2e:ui
```

## Usage Guide

### 1. Getting Started

When you first open the app, you'll see a "Get Started" screen. Click "Generate Demo Keypairs" to create test wallets for the demo.

### 2. Creating a Swap

After generating keypairs, fill in the swap form:
- **Asset A Contract ID**: The Stellar asset contract for the creator's asset
- **Amount A**: Amount of asset A to swap
- **Asset B Contract ID**: The Stellar asset contract for the counterparty's asset
- **Amount B**: Amount of asset B to swap
- **Expiry**: Time in seconds until the swap expires

Click "Create Swap" to initiate the swap.

### 3. Funding the Swap

Once created, both parties need to fund their sides:
1. Click "Fund Creator Side" to deposit asset A
2. Click "Fund Counterparty Side" to deposit asset B

The swap will show as "Funded" once both sides are deposited.

### 4. Accepting the Swap

When both sides are funded, the counterparty can click "Accept Swap" to execute the atomic transfer. Assets will be transferred instantly between parties.

### 5. Cancelling a Swap

If the swap hasn't been funded yet, the creator can cancel it by clicking "Cancel Swap".

## Architecture

### Component Structure

```
src/
├── App.tsx              # Main application component
├── main.tsx             # Entry point
└── index.css            # Global styles
```

### Key Features

- **State Management**: React hooks for managing swap state and UI state
- **SDK Integration**: Uses `@swaptrade/sdk` for contract interactions
- **Error Handling**: User-friendly error messages for failed transactions
- **Loading States**: Visual feedback during transaction processing

## Network Configuration

### Localnet (Standalone)

For local development with Soroban standalone:

```env
VITE_RPC_URL=http://localhost:8000/soroban/rpc
VITE_NETWORK_PASSPHRASE=Standalone Network ; February 2017
```

### Testnet (Futurenet)

For testing on Stellar testnet:

```env
VITE_RPC_URL=https://rpc-futurenet.stellar.org
VITE_NETWORK_PASSPHRASE=Test SDF Network ; September 2015
```

### Mainnet

For production use:

```env
VITE_RPC_URL=https://rpc.mainnet.stellar.org
VITE_NETWORK_PASSPHRASE=Public Global Stellar Network ; September 2015
```

## Troubleshooting

### Transaction Failures

- Ensure your accounts have sufficient XLM for fees
- Verify trustlines are established for the assets being swapped
- Check that the contract is deployed and accessible

### Connection Issues

- Verify the RPC URL is correct for your network
- Ensure the Soroban node is running (for localnet)
- Check network connectivity

### Build Errors

- Clear node_modules and reinstall: `rm -rf node_modules && npm install`
- Ensure Node.js version is 18 or higher
- Check that all dependencies are installed

## Development

### Adding New Features

1. Add new components to `src/`
2. Update SDK calls in `App.tsx`
3. Add corresponding E2E tests in `tests/e2e/`
4. Update documentation

### Styling

The app uses TailwindCSS for styling. Add custom styles in `index.css` or use Tailwind utility classes in components.

## License

MIT
