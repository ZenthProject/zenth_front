interface DateSeparatorProps {
  date: Date;
}

export function DateSeparator({ date }: DateSeparatorProps) {
  const formatDate = (d: Date) => {
    const today = new Date();
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);

    const isToday = d.toDateString() === today.toDateString();
    const isYesterday = d.toDateString() === yesterday.toDateString();

    if (isToday) return "Aujourd'hui";
    if (isYesterday) return "Hier";

    return d.toLocaleDateString("fr-FR", {
      weekday: "long",
      day: "numeric",
      month: "long",
    });
  };

  return (
    <div className="flex items-center justify-center my-4">
      <div className="flex-1 h-px bg-gray-800" />
      <span className="px-4 text-xs text-gray-500 font-medium">
        {formatDate(date)}
      </span>
      <div className="flex-1 h-px bg-gray-800" />
    </div>
  );
}
