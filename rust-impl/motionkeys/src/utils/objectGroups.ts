// object groups and objects from the bridge, picked on nodes by id, and the objects' channels

// what a node saves to pick a group: its id, or its name when it was saved without one
export const groupKey = (group: any): string => (typeof group?.id === "string" && group.id ? group.id : group?.name);

// the group a node's saved pick means: the one with that id, else the first with that name (picks saved before groups had ids)
export const findGroup = (groups: any[], pick: string | undefined): any => (pick ? (groups.find((g) => groupKey(g) === pick) ?? groups.find((g) => g?.name === pick)) : undefined);

// the picker's options: each group's id, shown by its name
export const groupOptions = (groups: any[]): { value: string; label: string }[] => groups.filter((g) => typeof groupKey(g) === "string").map((g) => ({ value: groupKey(g), label: typeof g?.name === "string" ? g.name : groupKey(g) }));

// the object a node's saved pick means: the one with that id, else the first with that name (names typed in)
export const findObject = (objects: any[], pick: string | undefined): any => (pick ? (objects.find((o) => o?.id === pick) ?? objects.find((o) => o?.name === pick)) : undefined);

// the picker's options: each object's id, shown by its name
export const objectOptions = (objects: any[]): { value: string; label: string }[] => objects.filter((o) => typeof o?.id === "string").map((o) => ({ value: o.id, label: typeof o?.name === "string" ? o.name : o.id }));

// a channel's name then its axis, `Location Z`
export const channelTitle = (channel: any): string => (channel?.axis ? `${channel.name} ${channel.axis}` : `${channel?.name ?? ""}`);

// a channel's label where it stands alone: its group in front, `Arm › Location X`, except for the object's own channels
// (its first channel's group). a channel the bridge didn't list is its id, like Object::channel_label in scene_generics
export function channelLabel(object: any, id: string): string {
    const channels: any[] = Array.isArray(object?.channels) ? object.channels : [];
    const channel = channels.find((c) => c?.id === id);
    if (!channel) return id;
    return !channel.group || channel.group === channels[0]?.group ? channelTitle(channel) : `${channel.group} › ${channelTitle(channel)}`;
}
