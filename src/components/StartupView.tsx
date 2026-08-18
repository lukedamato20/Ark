import { ArkBrand } from "../ui/arkBrand";
import { ActivityIndicator } from "../ui/activityIndicator";

export function StartupView() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-background text-foreground">
      <div className="flex flex-col items-center gap-4" role="status" aria-label="Starting Ark">
        <ArkBrand className="scale-125" />
        <ActivityIndicator state="preparing" announce={false} />
      </div>
    </main>
  );
}
