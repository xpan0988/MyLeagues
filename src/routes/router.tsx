import { createHashRouter, Navigate } from "react-router-dom";
import { AppShell } from "../components/layout/AppShell";
import { CareerPage } from "../features/career/CareerPage";
import { ChampionProfilePage } from "../features/champions/ChampionProfilePage";
import { ChampionsPage } from "../features/champions/ChampionsPage";
import { HomePage } from "../features/home/HomePage";
import { MatchesPage } from "../features/matches/MatchesPage";
import { SettingsPage } from "../features/settings/SettingsPage";

export const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: <Navigate to="/home" replace /> },
      { path: "/home", element: <HomePage /> },
      { path: "/champions", element: <ChampionsPage /> },
      { path: "/champions/:championId", element: <ChampionProfilePage /> },
      { path: "/matches", element: <MatchesPage /> },
      { path: "/career", element: <CareerPage /> },
      { path: "/settings", element: <SettingsPage /> },
    ],
  },
]);

