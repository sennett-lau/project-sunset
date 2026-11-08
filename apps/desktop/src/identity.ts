// The one place the UI reads the product name and bundle id. See docs/architecture.md §2.7.
import identity from "../../../app-identity.json";

export const appIdentity: { productName: string; bundleId: string } = identity;
