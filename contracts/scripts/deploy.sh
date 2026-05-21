#!/usr/bin/env bash
# PLACEHOLDER — Full implementation in Issue #021 (testnet deployment)
#
# deploy.sh — ForgePass Soroban contract deployment script
# Usage: ./contracts/scripts/deploy.sh [testnet|mainnet]
#
# DEPLOYMENT ORDER IS ARCHITECTURALLY SIGNIFICANT.
# Later contracts reference the passport contract address during initialisation.
# Deploying out of order will cause initialisation to fail.
#
# Order:
# 1. passport
# 2. credential-store
# 3. trust-score
# 4. soulbound-nft
#
# Prerequisites:
# - Soroban CLI installed at version matching SDK 22.0.7
# - FORGEPASS_ADMIN_SECRET env var set to the ForgePass admin wallet secret key
# - STELLAR_NETWORK env var set to testnet or mainnet
#
# TODO in Issue #021:
# - Implement deploy step for each contract in order
# - Capture and write deployed addresses to contracts/deployments/$STELLAR_NETWORK.json
# - Verify each deployment with a read call before proceeding to next contract

set -euo pipefail

echo "PLACEHOLDER: deploy.sh not yet implemented — see Issue #021"
exit 1
