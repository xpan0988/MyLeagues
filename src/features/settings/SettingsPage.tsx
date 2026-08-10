import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { PageHeader } from "../../components/ui/PageHeader";
import { QueryState } from "../../components/ui/DataDisplay";
import { backend } from "../../lib/tauri";
import type { SettingsDto, UpdateSettingsInput } from "../../lib/types";

export function SettingsPage() {
  const query = useQuery({ queryKey: ["settings"], queryFn: backend.getSettings });
  return <div className="page"><PageHeader eyebrow="LOCAL CONFIGURATION" title="Settings" description="One Riot identity, backend-only API authentication, and official client launch settings." /><QueryState loading={query.isPending} error={query.error}>{query.data ? <SettingsForm key={JSON.stringify(query.data)} initial={query.data} /> : null}</QueryState></div>;
}

function SettingsForm({ initial }: { initial: SettingsDto }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState<UpdateSettingsInput>({ gameName: initial.gameName, tagLine: initial.tagLine, accountRegion: initial.accountRegion, platformRegion: initial.platformRegion, riotClientPath: initial.riotClientPath });
  const save = useMutation({ mutationFn: backend.updateSettings, onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["settings"] }) });
  const sync = useMutation({ mutationFn: backend.startSync, onSuccess: () => void queryClient.invalidateQueries() });
  const rebuild = useMutation({ mutationFn: backend.rebuildAggregates, onSuccess: () => void queryClient.invalidateQueries() });
  const clearStatic = useMutation({ mutationFn: backend.clearStaticCache, onSuccess: () => void queryClient.invalidateQueries() });
  const resetArchive = useMutation({ mutationFn: backend.resetLocalArchive, onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["home"] }) });
  const submit = (event: FormEvent) => { event.preventDefault(); save.mutate(form); };
  return <form className="settings-form" onSubmit={submit}>
    <section className="settings-section"><div><h2>Account</h2><p>The Riot ID resolves to a PUUID; mutable summoner names are never database identity keys.</p></div><div className="settings-fields"><label>Game name<input value={form.gameName} onChange={(event) => setForm({ ...form, gameName: event.target.value })} /></label><label>Tag line<input value={form.tagLine} onChange={(event) => setForm({ ...form, tagLine: event.target.value })} /></label><label>Account routing<select value={form.accountRegion} onChange={(event) => setForm({ ...form, accountRegion: event.target.value })}><option value="americas">Americas</option><option value="europe">Europe</option><option value="asia">Asia</option><option value="sea">SEA</option></select></label><label>Platform<select value={form.platformRegion} onChange={(event) => setForm({ ...form, platformRegion: event.target.value })}><option value="oc1">OCE</option><option value="na1">NA</option><option value="euw1">EUW</option><option value="eun1">EUNE</option><option value="kr">KR</option><option value="jp1">JP</option></select></label></div></section>
    <section className="settings-section"><div><h2>Riot API</h2><p>The secret is read only by Rust from RIOT_API_KEY and never appears in the React bundle.</p></div><div className="setting-status"><span className={`status-dot ${initial.apiKeyConfigured ? "status-online" : "status-offline"}`} /><strong>{initial.apiKeyConfigured ? "API key configured" : "API key missing"}</strong></div></section>
    <section className="settings-section"><div><h2>Official Client</h2><p>Provide the official Riot Client executable for this platform. The launcher starts only the configured executable.</p></div><label className="wide-field">Executable path<input placeholder="Official Riot Client executable path" value={form.riotClientPath ?? ""} onChange={(event) => setForm({ ...form, riotClientPath: event.target.value || null })} /></label></section>
    <section className="settings-section"><div><h2>Synchronization</h2><p>Successful facts remain available offline. Interrupted work resumes from the persistent match queue.</p></div><button className="secondary-button" type="button" onClick={() => sync.mutate()} disabled={sync.isPending}>{sync.isPending ? "Starting…" : "Sync Now"}</button></section>
    <section className="settings-section"><div><h2>Static Data</h2><p>Data Dragon metadata is version-aware and cached locally.</p></div><strong>{initial.dataDragonVersion ?? "No cached version"}</strong></section>
    <section className="settings-section developer"><div><h2>Maintenance</h2><p>Aggregate caches can be rebuilt from normalized facts. Static metadata is re-downloaded when available.</p></div><div className="button-group"><button className="secondary-button" type="button" onClick={() => { if (window.confirm("Rebuild aggregate caches from normalized match facts?")) rebuild.mutate(); }}>Rebuild aggregates</button><button className="secondary-button danger" type="button" onClick={() => { if (window.confirm("Clear cached Data Dragon metadata? Dashboard names and icons may be unavailable until refresh.")) clearStatic.mutate(); }}>Clear static cache</button><button className="secondary-button danger" type="button" onClick={() => { if (window.confirm("Reset Local Archive? Downloaded match and stat history will be removed and re-downloaded from Riot. Your Riot account, API-key setup, client path, and app settings will be preserved.")) resetArchive.mutate(); }} disabled={resetArchive.isPending}>{resetArchive.isPending ? "Resetting…" : "Reset Local Archive"}</button></div></section>
    <footer className="settings-footer"><span>{save.isSuccess ? "Saved locally." : save.error ? String(save.error) : "Changes stay on this device."}</span><button className="primary-button" type="submit" disabled={save.isPending}>{save.isPending ? "SAVING" : "SAVE SETTINGS"}</button></footer>
  </form>;
}
