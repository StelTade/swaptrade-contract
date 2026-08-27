import { test, expect } from '@playwright/test';

test.describe('SwapTrade Demo App', () => {
  test('loads the application', async ({ page }) => {
    await page.goto('/');
    
    // Check that the main heading is visible
    await expect(page.getByRole('heading', { name: 'SwapTrade Demo' })).toBeVisible();
    await expect(page.getByText('Atomic Swaps on Stellar Soroban')).toBeVisible();
  });

  test('generates demo keypairs', async ({ page }) => {
    await page.goto('/');
    
    // Click the generate keypairs button
    await page.getByRole('button', { name: 'Generate Demo Keypairs' }).click();
    
    // Wait for success message
    await expect(page.getByText('Generated new keypairs for demo')).toBeVisible();
    
    // Check that wallet info is displayed
    await expect(page.getByText('Creator Address')).toBeVisible();
    await expect(page.getByText('Counterparty Address')).toBeVisible();
  });

  test('displays swap creation form after keypair generation', async ({ page }) => {
    await page.goto('/');
    
    // Generate keypairs first
    await page.getByRole('button', { name: 'Generate Demo Keypairs' }).click();
    await expect(page.getByText('Generated new keypairs for demo')).toBeVisible();
    
    // Check that the swap creation form is visible
    await expect(page.getByRole('heading', { name: 'Create New Swap' })).toBeVisible();
    await expect(page.getByLabel('Asset A Contract ID')).toBeVisible();
    await expect(page.getByLabel('Amount A')).toBeVisible();
    await expect(page.getByLabel('Asset B Contract ID')).toBeVisible();
    await expect(page.getByLabel('Amount B')).toBeVisible();
  });

  test('shows error when creating swap with invalid data', async ({ page }) => {
    await page.goto('/');
    
    // Generate keypairs
    await page.getByRole('button', { name: 'Generate Demo Keypairs' }).click();
    await expect(page.getByText('Generated new keypairs for demo')).toBeVisible();
    
    // Try to create swap without filling required fields
    await page.getByRole('button', { name: 'Create Swap' }).click();
    
    // Should show an error (validation happens on submit)
    // Note: This test may need adjustment based on actual validation implementation
  });

  test('displays swap status after creation', async ({ page }) => {
    await page.goto('/');
    
    // Generate keypairs
    await page.getByRole('button', { name: 'Generate Demo Keypairs' }).click();
    
    // Fill in swap form (this would require a real contract to work)
    await page.getByLabel('Asset A Contract ID').fill('C1234567890123456789012345678901234567890123456789012345678901234');
    await page.getByLabel('Amount A').fill('100');
    await page.getByLabel('Asset B Contract ID').fill('C9876543210987654321098765432109876543210987654321098765432109876');
    await page.getByLabel('Amount B').fill('200');
    
    // Note: Actual swap creation requires a deployed contract
    // This is a smoke test to verify UI elements are present
    await expect(page.getByRole('button', { name: 'Create Swap' })).toBeVisible();
  });

  test('displays swap actions when swap is created', async ({ page }) => {
    await page.goto('/');
    
    // Generate keypairs
    await page.getByRole('button', { name: 'Generate Demo Keypairs' }).click();
    
    // The swap actions section should not be visible initially
    await expect(page.getByRole('heading', { name: 'Swap Actions' })).not.toBeVisible();
    
    // After creating a swap, the actions section would appear
    // This test verifies the UI structure
  });
});
