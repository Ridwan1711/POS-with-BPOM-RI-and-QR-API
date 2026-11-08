import { expect, test, type Page } from "@playwright/test";

/**
 * End-to-end: SCAN -> IDENTIFY -> REVIEW -> SET PRICE -> SAVE -> POS SCAN -> PAY -> HISTORY.
 * Runs against a freshly seeded database. The barcode below is in the mock catalog.
 */
const PASSWORD = process.env.E2E_PASSWORD ?? "ChangeMe123!";
const MOCK_BARCODE = "8991002101029";

async function signIn(page: Page, email: string) {
  await page.goto("/login");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(/\/pos$/);
}

test("manager identifies a barcode, saves the product, and the cashier sells it", async ({ page }) => {
  await signIn(page, "manager@example.com");

  // 1. Identify through the external mock source and review the autofill.
  await page.goto("/products/new");
  await page.getByLabel("Barcode or QR code").fill(MOCK_BARCODE);
  await page.getByLabel("Barcode or QR code").press("Enter");
  await expect(page.getByText("Product found in external data", { exact: true })).toBeVisible();
  await expect(page.getByText("Filled in from")).toBeVisible();
  await expect(page.getByLabel("Product name")).toHaveValue("[MOCK] Mi Instan Goreng 85 g");
  await expect(page.getByLabel("Brand")).toHaveValue("Mock Brand Mie");
  await expect(page.getByLabel("BPOM registration number")).toHaveValue("MOCK-REG-0001");
  await expect(page.getByLabel("Registration status")).toHaveValue("reported");

  // 2. Merchant sets the prices and stock; external data never sets these.
  await page.getByLabel("Purchase price (IDR)").fill("2800");
  await page.getByLabel("Selling price (IDR)").fill("3500");
  await page.getByLabel("Initial stock").fill("30");
  await page.getByRole("button", { name: "Save product" }).click();

  await expect(page).toHaveURL(/\/products\/[0-9a-f-]{36}\?created=1$/);
  await expect(page.getByText("Product saved")).toBeVisible();
  await expect(page.getByText("Confirmed from", { exact: false })).toBeVisible();

  // 3. Cashier scans the new product, takes cash, and receives change.
  await signIn(page, "kasir@example.com");
  await page.goto("/pos");
  const scan = page.getByLabel("Scan barcode");
  await scan.fill(MOCK_BARCODE);
  await scan.press("Enter");
  await expect(page.getByText("Added [MOCK] Mi Instan Goreng 85 g")).toBeVisible();
  await expect(scan).toHaveValue("", { timeout: 5_000 });

  await page.getByRole("button", { name: /^Pay / }).click();
  await page.getByLabel("Cash received (IDR)").fill("5000");
  await page.getByRole("button", { name: /Complete sale/ }).click();

  const receiptHeading = page.getByRole("heading", { name: "Sale complete" });
  await expect(receiptHeading).toBeVisible();
  await expect(page.getByText("Change Rp 1.500")).toBeVisible();
  await page.getByRole("button", { name: "New sale" }).click();

  // 4. The sale is in history with its receipt.
  await page.goto("/transactions");
  await expect(page.getByRole("link", { name: /^\d{8}-\d{6}$/ }).first()).toBeVisible();
});

test("a cashier cannot reach the product creation screen", async ({ page }) => {
  await signIn(page, "kasir@example.com");
  await page.goto("/products/new");
  await expect(page.getByText("Managers only")).toBeVisible();
  await expect(page.getByLabel("Product name")).toHaveCount(0);
});

test("wrong password shows a generic message", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Email").fill("manager@example.com");
  await page.getByLabel("Password").fill("not-the-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByText("Invalid email or password.")).toBeVisible();
});
