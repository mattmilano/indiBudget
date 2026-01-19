/**
 * SimpleFIN Integration Service
 *
 * SimpleFIN is a privacy-focused bank aggregation service where:
 * - Users sign up at simplefin.org (~$1.50/month)
 * - Users connect their banks in SimpleFIN's dashboard
 * - Users get an access URL to paste into indiBudget
 * - indiBudget fetches transactions directly from SimpleFIN
 *
 * This keeps bank credentials out of indiBudget entirely.
 */

import { fetch } from '@tauri-apps/plugin-http';
import type {
  SimpleFINConfig,
  SimpleFINResponse,
  SimpleFINAccount,
  SimpleFINAccountMapping,
  SimpleFINSyncResult,
  CreateTransactionRequest,
} from '../types';
import * as api from './api';

const SIMPLEFIN_CONFIG_KEY = 'indibudget_simplefin_config';

/**
 * Get the current SimpleFIN configuration from localStorage
 */
export function getConfig(): SimpleFINConfig {
  try {
    const stored = localStorage.getItem(SIMPLEFIN_CONFIG_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error('Failed to load SimpleFIN config:', e);
  }

  return {
    accessUrl: null,
    lastSync: null,
    autoSync: false,
    syncInterval: 'manual',
    accountMappings: [],
  };
}

/**
 * Save SimpleFIN configuration to localStorage
 */
export function saveConfig(config: SimpleFINConfig): void {
  try {
    localStorage.setItem(SIMPLEFIN_CONFIG_KEY, JSON.stringify(config));
  } catch (e) {
    console.error('Failed to save SimpleFIN config:', e);
  }
}

/**
 * Clear SimpleFIN configuration (disconnect)
 */
export function clearConfig(): void {
  localStorage.removeItem(SIMPLEFIN_CONFIG_KEY);
}

/**
 * Parse the SimpleFIN access URL to extract credentials
 * Access URL format: https://TOKEN@beta-bridge.simplefin.org/simplefin
 */
function parseAccessUrl(accessUrl: string): { baseUrl: string; token: string } | null {
  try {
    // SimpleFIN provides a URL like: https://TOKEN@beta-bridge.simplefin.org/simplefin
    const url = new URL(accessUrl);
    const token = url.username;

    if (!token) {
      return null;
    }

    // Reconstruct URL without credentials for the base URL
    url.username = '';
    url.password = '';

    return {
      baseUrl: url.toString(),
      token,
    };
  } catch (e) {
    console.error('Invalid SimpleFIN access URL:', e);
    return null;
  }
}

/**
 * Fetch accounts and transactions from SimpleFIN
 */
export async function fetchAccounts(accessUrl: string): Promise<SimpleFINResponse> {
  const parsed = parseAccessUrl(accessUrl);

  if (!parsed) {
    throw new Error('Invalid SimpleFIN access URL. Please check the URL and try again.');
  }

  const { baseUrl, token } = parsed;
  const accountsUrl = `${baseUrl}accounts`;

  try {
    const response = await fetch(accountsUrl, {
      method: 'GET',
      headers: {
        'Authorization': `Basic ${btoa(token + ':')}`,
        'Accept': 'application/json',
      },
    });

    if (!response.ok) {
      if (response.status === 401) {
        throw new Error('SimpleFIN authentication failed. Your access URL may have expired. Please get a new one from simplefin.org.');
      }
      if (response.status === 403) {
        throw new Error('SimpleFIN access denied. Please check your subscription status at simplefin.org.');
      }
      throw new Error(`SimpleFIN request failed: ${response.status} ${response.statusText}`);
    }

    const data = await response.json() as SimpleFINResponse;
    return data;
  } catch (e) {
    if (e instanceof Error) {
      throw e;
    }
    throw new Error('Failed to connect to SimpleFIN. Please check your internet connection.');
  }
}

/**
 * Test the SimpleFIN connection with the given access URL
 */
export async function testConnection(accessUrl: string): Promise<{
  success: boolean;
  accounts: SimpleFINAccount[];
  error?: string;
}> {
  try {
    const response = await fetchAccounts(accessUrl);

    if (response.errors && response.errors.length > 0) {
      return {
        success: false,
        accounts: [],
        error: response.errors.join(', '),
      };
    }

    return {
      success: true,
      accounts: response.accounts || [],
    };
  } catch (e) {
    return {
      success: false,
      accounts: [],
      error: e instanceof Error ? e.message : 'Unknown error',
    };
  }
}

/**
 * Format a Unix timestamp to YYYY-MM-DD
 */
function formatDate(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  return date.toISOString().split('T')[0];
}

/**
 * Sync transactions from SimpleFIN to indiBudget
 */
export async function syncTransactions(
  accessUrl: string,
  accountMappings: SimpleFINAccountMapping[],
  onProgress?: (message: string) => void
): Promise<SimpleFINSyncResult> {
  const result: SimpleFINSyncResult = {
    accountsFound: 0,
    transactionsImported: 0,
    transactionsSkipped: 0,
    errors: [],
  };

  onProgress?.('Connecting to SimpleFIN...');

  let response: SimpleFINResponse;
  try {
    response = await fetchAccounts(accessUrl);
  } catch (e) {
    result.errors.push(e instanceof Error ? e.message : 'Failed to fetch from SimpleFIN');
    return result;
  }

  if (response.errors && response.errors.length > 0) {
    result.errors.push(...response.errors);
  }

  result.accountsFound = response.accounts?.length || 0;

  if (!response.accounts || response.accounts.length === 0) {
    result.errors.push('No accounts found in SimpleFIN');
    return result;
  }

  // Process each account
  for (const sfAccount of response.accounts) {
    const mapping = accountMappings.find(m => m.simplefinAccountId === sfAccount.id);

    if (!mapping || !mapping.indibudgetAccountId) {
      // Account not mapped, skip
      continue;
    }

    onProgress?.(`Syncing ${sfAccount.name}...`);

    // Process transactions for this account
    for (const sfTx of sfAccount.transactions) {
      try {
        // Skip pending transactions
        if (sfTx.pending) {
          result.transactionsSkipped++;
          continue;
        }

        // Determine transaction type based on amount
        const amount = parseFloat(sfTx.amount);
        const transactionType = amount >= 0 ? 'income' : 'expense';
        const absAmount = Math.abs(amount).toFixed(2);

        // Create the transaction
        const txRequest: CreateTransactionRequest = {
          account_id: mapping.indibudgetAccountId,
          transaction_type: transactionType,
          amount: absAmount,
          date: formatDate(sfTx.posted),
          description: sfTx.description,
          payee: sfTx.payee,
          notes: sfTx.memo ? `[SimpleFIN] ${sfTx.memo}` : '[SimpleFIN Import]',
          status: 'cleared',
        };

        // Import the transaction with duplicate detection
        const importResult = await api.importSingleTransaction(txRequest, `simplefin:${sfTx.id}`);

        if (importResult.imported) {
          result.transactionsImported++;
        } else if (importResult.duplicate) {
          result.transactionsSkipped++;
        }

      } catch (e) {
        const errorMsg = e instanceof Error ? e.message : 'Unknown error';
        result.errors.push(`Failed to import transaction "${sfTx.description}": ${errorMsg}`);
      }
    }
  }

  // Update last sync time
  const config = getConfig();
  config.lastSync = new Date().toISOString();
  saveConfig(config);

  onProgress?.('Sync complete!');

  return result;
}

/**
 * Create default account mappings from SimpleFIN accounts
 */
export function createDefaultMappings(accounts: SimpleFINAccount[]): SimpleFINAccountMapping[] {
  return accounts.map(acc => ({
    simplefinAccountId: acc.id,
    simplefinAccountName: acc.name,
    simplefinInstitution: acc.org?.name || 'Unknown',
    indibudgetAccountId: null, // User needs to map these
  }));
}

/**
 * Check if auto-sync should run based on config
 */
export function shouldAutoSync(config: SimpleFINConfig): boolean {
  if (!config.autoSync || !config.accessUrl) {
    return false;
  }

  if (!config.lastSync) {
    return true;
  }

  const lastSync = new Date(config.lastSync);
  const now = new Date();
  const diffHours = (now.getTime() - lastSync.getTime()) / (1000 * 60 * 60);

  switch (config.syncInterval) {
    case 'daily':
      return diffHours >= 24;
    case 'weekly':
      return diffHours >= 168; // 7 days
    default:
      return false;
  }
}
