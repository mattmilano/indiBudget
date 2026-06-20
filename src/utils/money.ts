/**
 * Money utilities using decimal.js for precise arithmetic
 *
 * This module provides type-safe money operations that avoid
 * floating-point precision issues inherent in JavaScript's
 * native number type.
 */

import Decimal from 'decimal.js';

// Configure Decimal for currency (2 decimal places, round half up)
Decimal.set({ precision: 20, rounding: Decimal.ROUND_HALF_UP });

/**
 * Parse a money string into a Decimal
 * Returns Decimal.ZERO for invalid inputs
 */
export function parseMoney(value: string | number | null | undefined): Decimal {
  if (value === null || value === undefined || value === '') {
    return new Decimal(0);
  }
  try {
    return new Decimal(value);
  } catch {
    return new Decimal(0);
  }
}

/**
 * Add two money values
 */
export function addMoney(a: string | number, b: string | number): Decimal {
  return parseMoney(a).plus(parseMoney(b));
}

/**
 * Subtract money values (a - b)
 */
export function subtractMoney(a: string | number, b: string | number): Decimal {
  return parseMoney(a).minus(parseMoney(b));
}

/**
 * Multiply money by a factor
 */
export function multiplyMoney(amount: string | number, factor: string | number): Decimal {
  return parseMoney(amount).times(parseMoney(factor));
}

/**
 * Divide money by a divisor
 * Returns Decimal.ZERO if divisor is zero
 */
export function divideMoney(amount: string | number, divisor: string | number): Decimal {
  const div = parseMoney(divisor);
  if (div.isZero()) {
    return new Decimal(0);
  }
  return parseMoney(amount).dividedBy(div);
}

/**
 * Sum an array of money values
 */
export function sumMoney(values: (string | number)[]): Decimal {
  return values.reduce((acc, val) => acc.plus(parseMoney(val)), new Decimal(0));
}

/**
 * Format a Decimal as a currency string (2 decimal places)
 */
export function formatDecimal(value: Decimal, places: number = 2): string {
  return value.toFixed(places);
}

/**
 * Compare two money values
 * Returns: -1 if a < b, 0 if a == b, 1 if a > b
 */
export function compareMoney(a: string | number, b: string | number): number {
  return parseMoney(a).comparedTo(parseMoney(b));
}

/**
 * Check if a money value is zero
 */
export function isZero(value: string | number): boolean {
  return parseMoney(value).isZero();
}

/**
 * Check if a money value is positive
 */
export function isPositive(value: string | number): boolean {
  return parseMoney(value).isPositive() && !parseMoney(value).isZero();
}

/**
 * Check if a money value is negative
 */
export function isNegative(value: string | number): boolean {
  return parseMoney(value).isNegative();
}

/**
 * Get absolute value
 */
export function absMoney(value: string | number): Decimal {
  return parseMoney(value).abs();
}

/**
 * Calculate percentage: (part / total) * 100
 * Returns 0 if total is zero
 */
export function percentage(part: string | number, total: string | number): Decimal {
  const totalDec = parseMoney(total);
  if (totalDec.isZero()) {
    return new Decimal(0);
  }
  return parseMoney(part).dividedBy(totalDec).times(100);
}

/**
 * Convert Decimal to number (use sparingly, only for display)
 */
export function toNumber(value: Decimal): number {
  return value.toNumber();
}
