import {
  BarChart3,
  CircleUserRound,
  History,
  Home,
  Settings,
  Swords,
} from "lucide-react";
import { NavLink, Outlet } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { backend } from "../../lib/tauri";

const navigation = [
  { to: "/home", label: "Home", icon: Home },
  { to: "/champions", label: "Champions", icon: Swords },
  { to: "/matches", label: "Matches", icon: History },
  { to: "/career", label: "Career", icon: BarChart3 },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

export function AppShell() {
  const home = useQuery({ queryKey: ["home-shell"], queryFn: backend.getHome, staleTime: 15_000 });
  const account = home.data?.account;
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">ML</span>
          <span>MYLEAGUE</span>
        </div>
        <nav className="primary-nav" aria-label="Primary navigation">
          {navigation.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                `nav-link${isActive ? " nav-link-active" : ""}`
              }
            >
              <Icon size={18} strokeWidth={1.7} />
              <span>{label}</span>
            </NavLink>
          ))}
        </nav>
        <div className="account-chip">
          <CircleUserRound size={20} strokeWidth={1.6} />
          <div>
            <span className="account-chip-label">LOCAL PROFILE</span>
            <span>{account ? `${account.gameName}#${account.tagLine}` : "Not configured"}</span>
          </div>
        </div>
      </aside>
      <main className="main-content">
        <Outlet />
      </main>
    </div>
  );
}
