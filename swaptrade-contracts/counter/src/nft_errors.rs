/// NFT-specific errors for the SwapTrade NFT marketplace
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum NFTError {
    // ===== General Errors (1000-1099) =====
    /// NFT not found
    NFTNotFound = 1000,
    /// Collection not found
    CollectionNotFound = 1001,
    /// Unauthorized operation
    Unauthorized = 1002,
    /// Invalid token ID
    InvalidTokenId = 1003,
    /// Invalid collection ID
    InvalidCollectionId = 1004,
    /// Invalid amount specified
    InvalidAmount = 1005,
    /// Invalid price specified
    InvalidPrice = 1006,
    /// Operation not supported for this NFT type
    UnsupportedOperation = 1007,
    /// NFT marketplace is paused
    MarketplacePaused = 1008,
    /// Contract is frozen for this user
    UserFrozen = 1009,

    // ===== Ownership Errors (1100-1199) =====
    /// Caller is not the owner
    NotOwner = 1100,
    /// Caller is not the creator
    NotCreator = 1101,
    /// Caller is not approved
    NotApproved = 1102,
    /// NFT already owned
    AlreadyOwned = 1103,
    /// Insufficient balance for ERC-1155
    InsufficientBalance = 1104,
    /// Token does not exist
    TokenDoesNotExist = 1105,

    // ===== Minting Errors (1200-1299) =====
    /// Minting is not active for this collection
    MintingNotActive = 1200,
    /// Collection has reached max supply
    MaxSupplyReached = 1201,
    /// Invalid metadata URI
    InvalidMetadata = 1202,
    /// Collection name already exists
    CollectionNameExists = 1203,
    /// Invalid royalty percentage
    InvalidRoyalty = 1204,

    // ===== Marketplace Errors (1300-1399) =====
    /// Listing not found
    ListingNotFound = 1300,
    /// Offer not found
    OfferNotFound = 1301,
    /// Listing is not active
    ListingNotActive = 1302,
    /// Offer is not active
    OfferNotActive = 1303,
    /// Listing has expired
    ListingExpired = 1304,
    /// Offer has expired
    OfferExpired = 1305,
    /// Invalid bid amount
    InvalidBid = 1306,
    /// Auction has ended
    AuctionEnded = 1307,
    /// Auction is still active
    AuctionActive = 1308,
    /// Self-dealing not allowed
    SelfDealing = 1309,
    /// Insufficient funds for purchase
    InsufficientFunds = 1310,
    /// Price slippage exceeded
    SlippageExceeded = 1311,

    // ===== Fractional NFT Errors (1400-1499) =====
    /// NFT is already fractionalized
    AlreadyFractionalized = 1400,
    /// NFT is not fractionalized
    NotFractionalized = 1401,
    /// No fractions available
    NoFractionsAvailable = 1402,
    /// Invalid share amount
    InvalidShareAmount = 1403,
    /// Fractionalization limit exceeded
    FractionalizationLimit = 1404,
    /// Cannot transfer full NFT while fractionalized
    TransferWhileFractionalized = 1405,

    // ===== Loan/Collateral Errors (1500-1599) =====
    /// Loan not found
    LoanNotFound = 1500,
    /// NFT is already used as collateral
    AlreadyCollateralized = 1501,
    /// NFT is not collateralized
    NotCollateralized = 1502,
    /// Loan is not active
    LoanNotActive = 1503,
    /// Loan is already repaid
    LoanAlreadyRepaid = 1504,
    /// Loan has been liquidated
    LoanLiquidated = 1505,
    /// Loan is not overdue
    LoanNotOverdue = 1506,
    /// Invalid interest rate
    InvalidInterestRate = 1507,
    /// Invalid loan duration
    InvalidDuration = 1508,
    /// Cannot liquidate active loan
    CannotLiquidate = 1509,
    /// Insufficient repayment amount
    InsufficientRepayment = 1510,
    /// Loan is not undercollateralized
    LoanNotUndercollateralized = 1511,
    /// Liquidation queue is full
    LiqQueueFull = 1512,
    /// No bids in liquidation auction
    NoAuctionBids = 1513,
    /// Invalid liquidation bid
    InvalidLiquidationBid = 1514,
    /// Interest calculation overflow
    InterestOverflow = 1515,

    // ===== Royalty Errors (1600-1699) =====
    /// Royalty payment failed
    RoyaltyPaymentFailed = 1600,
    /// Invalid royalty recipient
    InvalidRoyaltyRecipient = 1601,
    /// Royalty exceeds maximum
    ExcessiveRoyalty = 1602,

    // ===== Cross-chain Errors (1700-1799) =====
    /// Invalid chain ID
    InvalidChainId = 1700,
    /// Bridge not available
    BridgeNotAvailable = 1701,
    /// Cross-chain transfer failed
    CrossChainFailed = 1702,
    /// Wrapped NFT not found
    WrappedNFTNotFound = 1703,

    // ===== Valuation Errors (1800-1899) =====
    /// Valuation not available
    ValuationNotAvailable = 1800,
    /// Invalid valuation method
    InvalidValuationMethod = 1801,
    /// Oracle price not available
    OraclePriceNotAvailable = 1802,
}
