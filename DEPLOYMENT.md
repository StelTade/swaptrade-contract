# Deployment Guide

This guide covers deploying the SwapTrade SDK and Demo App to different environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Local Development Setup](#local-development-setup)
- [Deploying the SDK](#deploying-the-sdk)
- [Deploying the Demo App](#deploying-the-demo-app)
- [Deploying the Smart Contract](#deploying-the-smart-contract)
- [CI/CD Configuration](#cicd-configuration)
- [Environment Variables](#environment-variables)

## Prerequisites

- Node.js 18+ and npm/yarn
- Rust and Cargo (for contract compilation)
- Soroban CLI (`stellar` command)
- Git

## Local Development Setup

### 1. Clone the Repository

```bash
git clone https://github.com/your-org/swaptrade-contract.git
cd swaptrade-contract
```

### 2. Install SDK Dependencies

```bash
cd sdk
npm install
```

### 3. Install Demo App Dependencies

```bash
cd ../demo-app
npm install
```

### 4. Build the Smart Contract

```bash
cd ../swaptrade-contracts/atomic-swap
stellar contract build
```

### 5. Start Localnet

```bash
# In a separate terminal
stellar network start standalone
```

### 6. Deploy Contract to Localnet

```bash
# Fund admin account
stellar keys fund admin --network standalone

# Deploy contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/atomic_swap.wasm \
  --network standalone \
  --source admin
```

Save the contract ID for configuration.

### 7. Configure Demo App

```bash
cd demo-app
cp .env.example .env
```

Edit `.env` with your contract ID:
```env
VITE_SWAP_CONTRACT_ID=your_deployed_contract_id
VITE_RPC_URL=http://localhost:8000/soroban/rpc
VITE_NETWORK_PASSPHRASE=Standalone Network ; February 2017
```

### 8. Run Demo App

```bash
npm run dev
```

Visit `http://localhost:3000`

## Deploying the SDK

### Building for Production

```bash
cd sdk
npm run build
```

The compiled SDK will be in the `dist/` directory.

### Publishing to npm

```bash
# Login to npm
npm login

# Publish
npm publish --access public
```

### Using the SDK in Other Projects

```bash
npm install @swaptrade/sdk
```

## Deploying the Demo App

### Building for Production

```bash
cd demo-app
npm run build
```

The built app will be in the `dist/` directory.

### Deploying to Netlify

```bash
# Install Netlify CLI
npm install -g netlify-cli

# Deploy
netlify deploy --prod
```

### Deploying to Vercel

```bash
# Install Vercel CLI
npm install -g vercel

# Deploy
vercel --prod
```

### Deploying to GitHub Pages

```bash
# Build with correct base path
npm run build

# Deploy using gh-pages
npm install -g gh-pages
gh-pages -d dist
```

## Deploying the Smart Contract

### Localnet Deployment

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/atomic_swap.wasm \
  --network standalone \
  --source admin
```

### Testnet Deployment

```bash
# Fund account on testnet
stellar keys fund <your_public_key> --network testnet

# Deploy to testnet
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/atomic_swap.wasm \
  --network testnet \
  --source <your_secret_key>
```

### Mainnet Deployment

```bash
# Ensure account has sufficient XLM
# Deploy to mainnet
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/atomic_swap.wasm \
  --network public \
  --source <your_secret_key>
```

## CI/CD Configuration

### GitHub Actions Example

Create `.github/workflows/deploy.yml`:

```yaml
name: Deploy

on:
  push:
    branches: [main]

jobs:
  build-sdk:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - run: cd sdk && npm install
      - run: cd sdk && npm run build

  build-demo:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - run: cd demo-app && npm install
      - run: cd demo-app && npm run build

  test-e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - run: cd demo-app && npm install
      - run: cd demo-app && npm run test:e2e
```

## Environment Variables

### SDK Configuration

The SDK doesn't require environment variables - configuration is passed at runtime.

### Demo App Configuration

| Variable | Description | Example |
|----------|-------------|---------|
| `VITE_SWAP_CONTRACT_ID` | Deployed contract ID | `C1234...` |
| `VITE_RPC_URL` | Soroban RPC endpoint | `http://localhost:8000/soroban/rpc` |
| `VITE_NETWORK_PASSPHRASE` | Stellar network passphrase | `Standalone Network ; February 2017` |

### Network-Specific Configurations

#### Localnet
```env
VITE_RPC_URL=http://localhost:8000/soroban/rpc
VITE_NETWORK_PASSPHRASE=Standalone Network ; February 2017
```

#### Testnet
```env
VITE_RPC_URL=https://rpc-futurenet.stellar.org
VITE_NETWORK_PASSPHRASE=Test SDF Network ; September 2015
```

#### Mainnet
```env
VITE_RPC_URL=https://rpc.mainnet.stellar.org
VITE_NETWORK_PASSPHRASE=Public Global Stellar Network ; September 2015
```

## Troubleshooting

### Contract Deployment Fails

- Ensure you have sufficient XLM for deployment fees
- Check that the WASM file is built correctly
- Verify network connectivity

### SDK Build Errors

- Clear node_modules: `rm -rf node_modules && npm install`
- Check Node.js version (must be 18+)
- Verify TypeScript configuration

### Demo App Build Errors

- Ensure SDK is built first
- Check that environment variables are set
- Verify all dependencies are installed

### Localnet Connection Issues

- Ensure Soroban standalone is running
- Check that port 8000 is available
- Verify RPC URL configuration

## Security Considerations

- Never commit private keys or secrets
- Use environment variables for sensitive configuration
- Enable HTTPS in production
- Implement proper authentication for production deployments
- Regular security audits of smart contracts

## Monitoring

### Monitoring Contract Activity

Use Stellar Explorer or custom indexing to monitor:
- Swap creation events
- Funding transactions
- Acceptance and cancellation events
- Error rates

### Monitoring Demo App

Set up monitoring for:
- Application uptime
- Error rates
- Transaction success rates
- User engagement metrics

## Backup and Recovery

### Contract Backup

- Store contract WASM files
- Document deployment parameters
- Keep track of contract upgrades

### Data Backup

- Regular backups of application data
- Database backups if using external storage
- Configuration backups

## Support

For deployment issues:
- Check the troubleshooting section
- Review GitHub issues
- Contact the development team
