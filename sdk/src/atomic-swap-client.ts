/**
 * Atomic Swap Client - Main SDK implementation
 */

import {
  Contract,
  Keypair,
  SorobanRpc,
  TransactionBuilder,
  xdr,
} from "@stellar/stellar-sdk";

import {
  SwapTradeSDKConfig,
  CreateSwapParams,
  FundSwapParams,
  AcceptSwapParams,
  CancelSwapParams,
  RefundSwapParams,
  TransactionResult,
  Swap,
  SwapState,
} from "./types";
import {
  toAddress,
  toU64,
  toI128,
  calculateExpiry,
  generateNonce,
  isValidAddress,
  isValidAmount,
  isValidExpiry,
} from "./helpers";

/**
 * Main SDK class for interacting with the Atomic Swap contract
 */
export class AtomicSwapClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private config: SwapTradeSDKConfig;

  constructor(config: SwapTradeSDKConfig) {
    this.config = config;
    this.contract = new Contract(config.contractId);
    this.server = new SorobanRpc.Server(config.network.rpcUrl);
  }

  /**
   * Build a create_swap transaction
   */
  async buildCreateSwap(params: CreateSwapParams, signerPublicKey: string): Promise<string> {
    this.validateCreateParams(params);

    const account = await this.server.getAccount(signerPublicKey);
    const txn = new TransactionBuilder(account, {
      fee: "100000",
      networkPassphrase: this.config.network.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          "create_swap",
          toAddress(params.creator),
          toAddress(params.counterparty),
          toAddress(params.asset_a),
          toI128(params.amount_a),
          toAddress(params.asset_b),
          toI128(params.amount_b),
          toU64(params.expiry),
          toU64(params.nonce),
        ),
      )
      .setTimeout(300)
      .build();

    return txn.toXDR();
  }

  /**
   * Build a fund_swap transaction
   */
  async buildFundSwap(params: FundSwapParams, signerPublicKey: string): Promise<string> {
    const account = await this.server.getAccount(signerPublicKey);
    const txn = new TransactionBuilder(account, {
      fee: "100000",
      networkPassphrase: this.config.network.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          "fund_swap",
          toU64(params.swap_id),
          toAddress(params.funder),
        ),
      )
      .setTimeout(300)
      .build();

    return txn.toXDR();
  }

  /**
   * Build an accept_swap transaction
   */
  async buildAcceptSwap(params: AcceptSwapParams, signerPublicKey: string): Promise<string> {
    const account = await this.server.getAccount(signerPublicKey);
    const txn = new TransactionBuilder(account, {
      fee: "100000",
      networkPassphrase: this.config.network.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          "accept_swap",
          toU64(params.swap_id),
          toAddress(params.acceptor),
        ),
      )
      .setTimeout(300)
      .build();

    return txn.toXDR();
  }

  /**
   * Build a cancel_swap transaction
   */
  async buildCancelSwap(params: CancelSwapParams, signerPublicKey: string): Promise<string> {
    const account = await this.server.getAccount(signerPublicKey);
    const txn = new TransactionBuilder(account, {
      fee: "100000",
      networkPassphrase: this.config.network.networkPassphrase,
    })
      .addOperation(
        this.contract.call("cancel_swap", toU64(params.swap_id)),
      )
      .setTimeout(300)
      .build();

    return txn.toXDR();
  }

  /**
   * Build a refund_swap transaction
   */
  async buildRefundSwap(params: RefundSwapParams, signerPublicKey: string): Promise<string> {
    const account = await this.server.getAccount(signerPublicKey);
    const txn = new TransactionBuilder(account, {
      fee: "100000",
      networkPassphrase: this.config.network.networkPassphrase,
    })
      .addOperation(
        this.contract.call("refund_swap", toU64(params.swap_id)),
      )
      .setTimeout(300)
      .build();

    return txn.toXDR();
  }

  /**
   * Sign and submit a transaction
   */
  async signAndSubmitTransaction(xdrTx: string, signer: Keypair): Promise<TransactionResult> {
    const tx = TransactionBuilder.fromXDR(xdrTx, this.config.network.networkPassphrase);
    tx.sign(signer);
    
    const result = await this.server.sendTransaction(tx);
    
    if (result.status === "ERROR") {
      throw new Error(`Transaction failed: ${JSON.stringify(result.errorResult)}`);
    }

    // Wait for confirmation
    const hash = result.hash;
    for (let i = 0; i < 10; i++) {
      const get = await this.server.getTransaction(hash);
      if (get.status === "SUCCESS") {
        return { hash, status: "SUCCESS" };
      }
      if (get.status === "FAILED") {
        return { hash, status: "FAILED" };
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }

    return { hash, status: "PENDING" };
  }

  /**
   * Get swap details
   */
  async getSwap(swapId: number): Promise<Swap> {
    const result = await this.server.simulateTransaction(
      new TransactionBuilder(new TransactionBuilder.BuildOptions(), {
        fee: "100",
        networkPassphrase: this.config.network.networkPassphrase,
      })
        .addOperation(
          this.contract.call("get_swap", toU64(swapId)),
        )
        .setTimeout(300)
        .build(),
    );

    if (result.result?.retval) {
      return this.parseSwapResult(result.result.retval);
    }

    throw new Error("Failed to fetch swap details");
  }

  /**
   * Check if an address has a trustline for an asset
   */
  async checkTrustline(address: string, asset: string): Promise<boolean> {
    const result = await this.server.simulateTransaction(
      new TransactionBuilder(new TransactionBuilder.BuildOptions(), {
        fee: "100",
        networkPassphrase: this.config.network.networkPassphrase,
      })
        .addOperation(
          this.contract.call("check_trustline", toAddress(address), toAddress(asset)),
        )
        .setTimeout(300)
        .build(),
    );

    if (result.result?.retval) {
      return result.result.retval?.value() === true;
    }

    return false;
  }

  /**
   * Get minimum expiry window
   */
  async getMinExpiry(): Promise<number> {
    const result = await this.server.simulateTransaction(
      new TransactionBuilder(new TransactionBuilder.BuildOptions(), {
        fee: "100",
        networkPassphrase: this.config.network.networkPassphrase,
      })
        .addOperation(
          this.contract.call("get_min_expiry"),
        )
        .setTimeout(300)
        .build(),
    );

    if (result.result?.retval) {
      return Number(result.result.retval?.value());
    }

    return 300; // Default
  }

  /**
   * Helper method to create swap with validation
   */
  async createSwap(
    creator: Keypair,
    counterparty: string,
    assetA: string,
    amountA: number,
    assetB: string,
    amountB: number,
    expirySeconds: number,
  ): Promise<{ swapId: number; txHash: string }> {
    const expiry = calculateExpiry(expirySeconds);
    const nonce = generateNonce();

    const params: CreateSwapParams = {
      creator: creator.publicKey(),
      counterparty,
      asset_a: assetA,
      amount_a: amountA,
      asset_b: assetB,
      amount_b: amountB,
      expiry,
      nonce,
    };

    const xdrTx = await this.buildCreateSwap(params, creator.publicKey());
    const result = await this.signAndSubmitTransaction(xdrTx, creator);

    if (result.status !== "SUCCESS") {
      throw new Error("Failed to create swap");
    }

    // Note: In a real implementation, you'd parse the result to get the swap ID
    // For now, we'll return a placeholder
    return { swapId: nonce, txHash: result.hash };
  }

  /**
   * Parse swap result from contract call
   */
  private parseSwapResult(scval: xdr.ScVal): Swap {
    // This is a simplified parser - in production you'd properly decode the XDR
    // For now, return a placeholder structure
    return {
      id: 0,
      nonce: 0,
      creator: "",
      counterparty: "",
      asset_a: "",
      amount_a: BigInt(0),
      asset_b: "",
      amount_b: BigInt(0),
      expiry: 0,
      state: SwapState.Created,
      creator_funded: false,
      counterparty_funded: false,
      created_at: 0,
    };
  }

  /**
   * Validate create swap parameters
   */
  private validateCreateParams(params: CreateSwapParams): void {
    if (!isValidAddress(params.creator)) {
      throw new Error("Invalid creator address");
    }
    if (!isValidAddress(params.counterparty)) {
      throw new Error("Invalid counterparty address");
    }
    if (!isValidAddress(params.asset_a)) {
      throw new Error("Invalid asset A address");
    }
    if (!isValidAddress(params.asset_b)) {
      throw new Error("Invalid asset B address");
    }
    if (!isValidAmount(params.amount_a)) {
      throw new Error("Amount A must be positive");
    }
    if (!isValidAmount(params.amount_b)) {
      throw new Error("Amount B must be positive");
    }
    if (params.asset_a === params.asset_b) {
      throw new Error("Asset A and B must be different");
    }
    if (params.creator === params.counterparty) {
      throw new Error("Creator and counterparty must be different");
    }
    if (!isValidExpiry(params.expiry)) {
      throw new Error("Expiry must be at least 300 seconds in the future");
    }
  }
}
