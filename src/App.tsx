import { Moon02Icon, Sun01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useTheme } from "@/hooks/use-theme";
import { ComingSoon } from "@/screens/coming-soon";
import { DnsLookup } from "@/screens/dns-lookup";
import { IpInfo } from "@/screens/ip-info";
import { LanScan } from "@/screens/lan-scan";
import { Ping } from "@/screens/ping";
import { PortCheck } from "@/screens/port-check";
import { SavedTargets } from "@/screens/saved-targets";
import { SubnetCalculator } from "@/screens/subnet-calculator";
import { Traceroute } from "@/screens/traceroute";
import { useAppStore } from "@/store";
import { SAVED_TARGETS, TOOLS, screenById, type Screen } from "@/tools";

function App() {
  const screen = screenById(useAppStore((state) => state.screen));

  return (
    <TooltipProvider>
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <header className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
            <SidebarTrigger className="-ml-1" />
            <Separator orientation="vertical" className="mr-2 data-[orientation=vertical]:h-4" />
            <h1 className="text-sm font-medium">{screen.name}</h1>
            <span className="truncate text-xs text-muted-foreground">{screen.summary}</span>
          </header>
          <main className="flex-1 overflow-auto p-6">
            <Notice />
            <ScreenContent screen={screen} />
          </main>
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  );
}

/** A message that stays until dismissed, e.g. why a Cross-Tool action did not start. */
function Notice() {
  const notice = useAppStore((state) => state.notice);
  const showNotice = useAppStore((state) => state.showNotice);
  if (notice === null) return null;
  return (
    <div role="status" className="mb-4 flex max-w-3xl items-center justify-between gap-3 rounded-md border px-3 py-2 text-xs">
      <span>{notice}</span>
      <Button type="button" size="xs" variant="ghost" onClick={() => showNotice(null)}>
        Dismiss
      </Button>
    </div>
  );
}

function ScreenContent({ screen }: { screen: Screen }) {
  switch (screen.id) {
    case "subnet-calculator":
      return <SubnetCalculator />;
    case "ping":
      return <Ping />;
    case "traceroute":
      return <Traceroute />;
    case "dns-lookup":
      return <DnsLookup />;
    case "ip-info":
      return <IpInfo />;
    case "port-check":
      return <PortCheck />;
    case "lan-scan":
      return <LanScan />;
    case "saved-targets":
      return <SavedTargets />;
    default:
      return <ComingSoon screen={screen} />;
  }
}

function AppSidebar() {
  const { theme, toggleTheme } = useTheme();

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <div className="px-2 py-1 text-sm font-semibold group-data-[collapsible=icon]:hidden">IT Tools</div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Tools</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {TOOLS.map((tool) => (
                <ScreenLink key={tool.id} screen={tool} />
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
        <SidebarGroup>
          <SidebarGroupLabel>Library</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <ScreenLink screen={SAVED_TARGETS} />
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton onClick={toggleTheme} tooltip="Toggle theme">
              <HugeiconsIcon icon={theme === "dark" ? Sun01Icon : Moon02Icon} strokeWidth={2} />
              <span>{theme === "dark" ? "Light theme" : "Dark theme"}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}

function ScreenLink({ screen }: { screen: Screen }) {
  const active = useAppStore((state) => state.screen === screen.id);
  const showScreen = useAppStore((state) => state.showScreen);

  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={active} tooltip={screen.name} onClick={() => showScreen(screen.id)}>
        <HugeiconsIcon icon={screen.icon} strokeWidth={2} />
        <span>{screen.name}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  );
}

export default App;
