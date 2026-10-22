import { invoke } from "@tauri-apps/api/core";
import type { AddressConfiguration } from "./network";

export interface ProfileSummary {
  id: string;
  name: string;
  description: string;
  adapterCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface ActivityEvent {
  id: string;
  occurredAt: string;
  severity: string;
  category: string;
  message: string;
  details: unknown;
}

export const listProfiles = () => invoke<ProfileSummary[]>("list_network_profiles");
export const createProfile = (name: string, description: string) => invoke<string>("create_network_profile", { name, description });
export const deleteProfile = (id: string) => invoke<void>("delete_network_profile", { id });
export const duplicateProfile = (id: string) => invoke<string>("duplicate_network_profile", { id });
export const updateProfile = (id: string, name: string, description: string) => invoke<void>("update_network_profile", { id, name, description });
export interface ProfileApplyItem { adapterName: string; interfaceIndex: number | null; routeRole: string; configuration: AddressConfiguration | null; warning: string | null }
export const getProfileApplyPlan = (id: string) => invoke<ProfileApplyItem[]>("get_profile_apply_plan", { id });
export const exportProfile = (id: string) => invoke<string>("export_network_profile", { id });
export const importProfile = (source: string) => invoke<string>("import_network_profile", { source });
export const listActivityEvents = (limit = 200) => invoke<ActivityEvent[]>("list_activity_events", { limit });

