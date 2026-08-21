import { useState } from 'react'
import { ArrowRightLeft, CheckCircle, XCircle, Clock, AlertCircle } from 'lucide-react'
import { AtomicSwapClient, Keypair, SwapState } from '@swaptrade/sdk'

function App() {
  const [isConnected, setIsConnected] = useState(false)
  const [creatorKey, setCreatorKey] = useState<Keypair | null>(null)
  const [counterpartyKey, setCounterpartyKey] = useState<Keypair | null>(null)
  const [swapId, setSwapId] = useState<number | null>(null)
  const [swapState, setSwapState] = useState<SwapState | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  const [formData, setFormData] = useState({
    assetA: '',
    amountA: '',
    assetB: '',
    amountB: '',
    counterparty: '',
    expiry: '3600',
  })

  const client = new AtomicSwapClient({
    contractId: import.meta.env.VITE_SWAP_CONTRACT_ID || '',
    network: {
      rpcUrl: import.meta.env.VITE_RPC_URL || 'http://localhost:8000/soroban/rpc',
      networkPassphrase: import.meta.env.VITE_NETWORK_PASSPHRASE || 'Standalone Network ; February 2017',
    },
  })

  const generateKeypairs = () => {
    const creator = Keypair.random()
    const counterparty = Keypair.random()
    setCreatorKey(creator)
    setCounterpartyKey(counterparty)
    setFormData(prev => ({ ...prev, counterparty: counterparty.publicKey() }))
    setIsConnected(true)
    setSuccess('Generated new keypairs for demo')
    setTimeout(() => setSuccess(null), 3000)
  }

  const createSwap = async () => {
    if (!creatorKey) return
    
    setLoading(true)
    setError(null)
    setSuccess(null)

    try {
      const result = await client.createSwap(
        creatorKey,
        formData.counterparty,
        formData.assetA,
        Number(formData.amountA),
        formData.assetB,
        Number(formData.amountB),
        Number(formData.expiry),
      )
      setSwapId(result.swapId)
      setSwapState(SwapState.Created)
      setSuccess(`Swap created successfully! ID: ${result.swapId}`)
      setTimeout(() => setSuccess(null), 3000)
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Failed to create swap')
    } finally {
      setLoading(false)
    }
  }

  const fundSwap = async (isCreator: boolean) => {
    if (!swapId || !creatorKey || !counterpartyKey) return
    
    setLoading(true)
    setError(null)
    setSuccess(null)

    try {
      const funder = isCreator ? creatorKey : counterpartyKey
      const xdrTx = await client.buildFundSwap(
        { swap_id: swapId, funder: funder.publicKey() },
        funder.publicKey(),
      )
      await client.signAndSubmitTransaction(xdrTx, funder)
      
      if (isCreator) {
        setSwapState(SwapState.Created)
        setSuccess('Creator funded their side')
      } else {
        setSwapState(SwapState.Funded)
        setSuccess('Counterparty funded their side - swap is now fully funded!')
      }
      setTimeout(() => setSuccess(null), 3000)
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Failed to fund swap')
    } finally {
      setLoading(false)
    }
  }

  const acceptSwap = async () => {
    if (!swapId || !counterpartyKey) return
    
    setLoading(true)
    setError(null)
    setSuccess(null)

    try {
      const xdrTx = await client.buildAcceptSwap(
        { swap_id: swapId, acceptor: counterpartyKey.publicKey() },
        counterpartyKey.publicKey(),
      )
      await client.signAndSubmitTransaction(xdrTx, counterpartyKey)
      setSwapState(SwapState.Accepted)
      setSuccess('Swap accepted - assets transferred atomically!')
      setTimeout(() => setSuccess(null), 3000)
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Failed to accept swap')
    } finally {
      setLoading(false)
    }
  }

  const cancelSwap = async () => {
    if (!swapId || !creatorKey) return
    
    setLoading(true)
    setError(null)
    setSuccess(null)

    try {
      const xdrTx = await client.buildCancelSwap(
        { swap_id: swapId },
        creatorKey.publicKey(),
      )
      await client.signAndSubmitTransaction(xdrTx, creatorKey)
      setSwapState(SwapState.Cancelled)
      setSuccess('Swap cancelled successfully')
      setTimeout(() => setSuccess(null), 3000)
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Failed to cancel swap')
    } finally {
      setLoading(false)
    }
  }

  const resetDemo = () => {
    setSwapId(null)
    setSwapState(null)
    setError(null)
    setSuccess(null)
    setFormData({
      assetA: '',
      amountA: '',
      assetB: '',
      amountB: '',
      counterparty: counterpartyKey?.publicKey() || '',
      expiry: '3600',
    })
  }

  const getStateIcon = (state: SwapState | null) => {
    switch (state) {
      case SwapState.Created:
        return <Clock className="w-5 h-5" />
      case SwapState.Funded:
        return <CheckCircle className="w-5 h-5" />
      case SwapState.Accepted:
        return <CheckCircle className="w-5 h-5 text-green-500" />
      case SwapState.Cancelled:
        return <XCircle className="w-5 h-5 text-red-500" />
      case SwapState.Refunded:
        return <AlertCircle className="w-5 h-5 text-yellow-500" />
      default:
        return null
    }
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-900 via-purple-900 to-slate-900">
      <div className="container mx-auto px-4 py-8">
        <div className="max-w-4xl mx-auto">
          <header className="text-center mb-12">
            <h1 className="text-4xl font-bold text-white mb-2 flex items-center justify-center gap-3">
              <ArrowRightLeft className="w-10 h-10 text-purple-400" />
              SwapTrade Demo
            </h1>
            <p className="text-slate-300">Atomic Swaps on Stellar Soroban</p>
          </header>

          {error && (
            <div className="mb-6 p-4 bg-red-500/10 border border-red-500/50 rounded-lg flex items-center gap-3">
              <AlertCircle className="w-5 h-5 text-red-400" />
              <p className="text-red-300">{error}</p>
            </div>
          )}

          {success && (
            <div className="mb-6 p-4 bg-green-500/10 border border-green-500/50 rounded-lg flex items-center gap-3">
              <CheckCircle className="w-5 h-5 text-green-400" />
              <p className="text-green-300">{success}</p>
            </div>
          )}

          {!isConnected ? (
            <div className="bg-slate-800/50 backdrop-blur-sm rounded-xl p-8 border border-slate-700">
              <h2 className="text-2xl font-semibold text-white mb-4">Get Started</h2>
              <p className="text-slate-300 mb-6">
                Generate demo keypairs to start creating atomic swaps. In production, you would connect your existing wallet.
              </p>
              <button
                onClick={generateKeypairs}
                className="w-full bg-purple-600 hover:bg-purple-700 text-white font-semibold py-3 px-6 rounded-lg transition-colors"
              >
                Generate Demo Keypairs
              </button>
            </div>
          ) : (
            <>
              <div className="bg-slate-800/50 backdrop-blur-sm rounded-xl p-6 border border-slate-700 mb-6">
                <h3 className="text-lg font-semibold text-white mb-4">Wallet Info</h3>
                <div className="grid md:grid-cols-2 gap-4">
                  <div>
                    <p className="text-slate-400 text-sm mb-1">Creator Address</p>
                    <p className="text-white font-mono text-sm break-all">{creatorKey?.publicKey()}</p>
                  </div>
                  <div>
                    <p className="text-slate-400 text-sm mb-1">Counterparty Address</p>
                    <p className="text-white font-mono text-sm break-all">{counterpartyKey?.publicKey()}</p>
                  </div>
                </div>
              </div>

              {swapState && (
                <div className="bg-slate-800/50 backdrop-blur-sm rounded-xl p-6 border border-slate-700 mb-6">
                  <h3 className="text-lg font-semibold text-white mb-4">Swap Status</h3>
                  <div className="flex items-center gap-3">
                    {getStateIcon(swapState)}
                    <span className="text-white font-medium">{swapState}</span>
                  </div>
                  {swapId && (
                    <p className="text-slate-400 mt-2">Swap ID: {swapId}</p>
                  )}
                </div>
              )}

              {!swapId ? (
                <div className="bg-slate-800/50 backdrop-blur-sm rounded-xl p-6 border border-slate-700">
                  <h3 className="text-lg font-semibold text-white mb-4">Create New Swap</h3>
                  <div className="space-y-4">
                    <div className="grid md:grid-cols-2 gap-4">
                      <div>
                        <label className="block text-slate-300 text-sm mb-2">Asset A Contract ID</label>
                        <input
                          type="text"
                          value={formData.assetA}
                          onChange={(e) => setFormData({ ...formData, assetA: e.target.value })}
                          className="w-full bg-slate-700 text-white rounded-lg px-4 py-2 border border-slate-600 focus:border-purple-500 focus:outline-none"
                          placeholder="C..."
                        />
                      </div>
                      <div>
                        <label className="block text-slate-300 text-sm mb-2">Amount A</label>
                        <input
                          type="number"
                          value={formData.amountA}
                          onChange={(e) => setFormData({ ...formData, amountA: e.target.value })}
                          className="w-full bg-slate-700 text-white rounded-lg px-4 py-2 border border-slate-600 focus:border-purple-500 focus:outline-none"
                          placeholder="100"
                        />
                      </div>
                    </div>
                    <div className="grid md:grid-cols-2 gap-4">
                      <div>
                        <label className="block text-slate-300 text-sm mb-2">Asset B Contract ID</label>
                        <input
                          type="text"
                          value={formData.assetB}
                          onChange={(e) => setFormData({ ...formData, assetB: e.target.value })}
                          className="w-full bg-slate-700 text-white rounded-lg px-4 py-2 border border-slate-600 focus:border-purple-500 focus:outline-none"
                          placeholder="C..."
                        />
                      </div>
                      <div>
                        <label className="block text-slate-300 text-sm mb-2">Amount B</label>
                        <input
                          type="number"
                          value={formData.amountB}
                          onChange={(e) => setFormData({ ...formData, amountB: e.target.value })}
                          className="w-full bg-slate-700 text-white rounded-lg px-4 py-2 border border-slate-600 focus:border-purple-500 focus:outline-none"
                          placeholder="200"
                        />
                      </div>
                    </div>
                    <div>
                      <label className="block text-slate-300 text-sm mb-2">Expiry (seconds)</label>
                      <input
                        type="number"
                        value={formData.expiry}
                        onChange={(e) => setFormData({ ...formData, expiry: e.target.value })}
                        className="w-full bg-slate-700 text-white rounded-lg px-4 py-2 border border-slate-600 focus:border-purple-500 focus:outline-none"
                        placeholder="3600"
                      />
                    </div>
                    <button
                      onClick={createSwap}
                      disabled={loading}
                      className="w-full bg-purple-600 hover:bg-purple-700 disabled:bg-purple-800 text-white font-semibold py-3 px-6 rounded-lg transition-colors"
                    >
                      {loading ? 'Creating...' : 'Create Swap'}
                    </button>
                  </div>
                </div>
              ) : (
                <div className="bg-slate-800/50 backdrop-blur-sm rounded-xl p-6 border border-slate-700">
                  <h3 className="text-lg font-semibold text-white mb-4">Swap Actions</h3>
                  <div className="space-y-3">
                    {swapState === SwapState.Created && (
                      <>
                        <button
                          onClick={() => fundSwap(true)}
                          disabled={loading}
                          className="w-full bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white font-semibold py-3 px-6 rounded-lg transition-colors"
                        >
                          {loading ? 'Funding...' : 'Fund Creator Side'}
                        </button>
                        <button
                          onClick={() => fundSwap(false)}
                          disabled={loading}
                          className="w-full bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white font-semibold py-3 px-6 rounded-lg transition-colors"
                        >
                          {loading ? 'Funding...' : 'Fund Counterparty Side'}
                        </button>
                        <button
                          onClick={cancelSwap}
                          disabled={loading}
                          className="w-full bg-red-600 hover:bg-red-700 disabled:bg-red-800 text-white font-semibold py-3 px-6 rounded-lg transition-colors"
                        >
                          {loading ? 'Cancelling...' : 'Cancel Swap'}
                        </button>
                      </>
                    )}
                    {swapState === SwapState.Funded && (
                      <button
                        onClick={acceptSwap}
                        disabled={loading}
                        className="w-full bg-green-600 hover:bg-green-700 disabled:bg-green-800 text-white font-semibold py-3 px-6 rounded-lg transition-colors"
                      >
                        {loading ? 'Accepting...' : 'Accept Swap (Execute)'}
                      </button>
                    )}
                    {swapState === SwapState.Accepted && (
                      <button
                        onClick={resetDemo}
                        className="w-full bg-slate-600 hover:bg-slate-700 text-white font-semibold py-3 px-6 rounded-lg transition-colors"
                      >
                        Create New Swap
                      </button>
                    )}
                    {swapState === SwapState.Cancelled && (
                      <button
                        onClick={resetDemo}
                        className="w-full bg-slate-600 hover:bg-slate-700 text-white font-semibold py-3 px-6 rounded-lg transition-colors"
                      >
                        Create New Swap
                      </button>
                    )}
                  </div>
                </div>
              )}
            </>
          )}

          <footer className="mt-12 text-center text-slate-400 text-sm">
            <p>Built with SwapTrade SDK • Stellar Soroban</p>
          </footer>
        </div>
      </div>
    </div>
  )
}

export default App
