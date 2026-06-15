import { useState, useEffect, useCallback } from "react";
import { Search, Command } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useNavigate } from "react-router-dom";
import { cn } from "@/lib/utils";

interface SearchResult {
  id: string;
  type: "page" | "friend" | "message" | "setting";
  title: string;
  description?: string;
  path?: string;
}

const defaultPages: SearchResult[] = [
  { id: "dashboard", type: "page", title: "Tableau de bord", path: "/dashboard" },
  { id: "chat", type: "page", title: "Messages", path: "/chat" },
  { id: "friends", type: "page", title: "Amis", path: "/friends" },
  { id: "settings", type: "page", title: "Parametres", path: "/settings" },
  { id: "network", type: "page", title: "Reseau", path: "/network" },
];

export function GlobalSearch() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>(defaultPages);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const navigate = useNavigate();

  // Filter results based on query
  useEffect(() => {
    if (!query.trim()) {
      setResults(defaultPages);
      return;
    }

    const filtered = defaultPages.filter(
      (item) =>
        item.title.toLowerCase().includes(query.toLowerCase()) ||
        item.description?.toLowerCase().includes(query.toLowerCase())
    );
    setResults(filtered);
    setSelectedIndex(0);
  }, [query]);

  // Keyboard shortcut to open search
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Handle navigation in results
  const handleKeyNavigation = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1) % results.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev - 1 + results.length) % results.length);
      } else if (e.key === "Enter" && results[selectedIndex]) {
        e.preventDefault();
        handleSelect(results[selectedIndex]);
      }
    },
    [results, selectedIndex]
  );

  const handleSelect = (result: SearchResult) => {
    if (result.path) {
      navigate(result.path);
    }
    setOpen(false);
    setQuery("");
  };

  return (
    <>
      {/* Trigger Button */}
      <Button
        variant="outline"
        className={cn(
          "relative h-9 w-full justify-start text-sm text-muted-foreground",
          "sm:w-64 sm:pr-12",
          "bg-secondary/50 border-border hover:bg-secondary"
        )}
        onClick={() => setOpen(true)}
      >
        <Search className="mr-2 h-4 w-4" />
        <span className="hidden sm:inline-flex">Rechercher...</span>
        <span className="inline-flex sm:hidden">Rechercher</span>
        <kbd
          className={cn(
            "pointer-events-none absolute right-1.5 top-1.5",
            "hidden h-6 select-none items-center gap-1",
            "rounded border border-border bg-muted px-1.5",
            "font-mono text-[10px] font-medium text-muted-foreground",
            "sm:flex"
          )}
        >
          <Command className="h-3 w-3" />K
        </kbd>
      </Button>

      {/* Search Dialog */}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-[550px] p-0 gap-0">
          <DialogHeader className="sr-only">
            <DialogTitle>Recherche globale</DialogTitle>
          </DialogHeader>
          <div className="flex items-center border-b px-3">
            <Search className="mr-2 h-4 w-4 shrink-0 text-muted-foreground" />
            <Input
              placeholder="Rechercher des pages, amis, messages..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyNavigation}
              className="flex h-12 w-full rounded-none border-0 bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:ring-0 focus-visible:ring-offset-0"
              autoFocus
            />
          </div>
          <div className="max-h-[300px] overflow-y-auto p-2">
            {results.length === 0 ? (
              <div className="py-6 text-center text-sm text-muted-foreground">
                Aucun resultat pour "{query}"
              </div>
            ) : (
              <div className="space-y-1">
                {results.map((result, index) => (
                  <button
                    key={result.id}
                    onClick={() => handleSelect(result)}
                    className={cn(
                      "w-full flex items-center gap-3 rounded-md px-3 py-2 text-sm",
                      "text-left transition-colors",
                      index === selectedIndex
                        ? "bg-accent text-accent-foreground"
                        : "hover:bg-accent/50"
                    )}
                  >
                    <span className="flex h-8 w-8 items-center justify-center rounded-md border bg-background">
                      <Search className="h-4 w-4 text-muted-foreground" />
                    </span>
                    <div className="flex flex-col">
                      <span className="font-medium">{result.title}</span>
                      {result.description && (
                        <span className="text-xs text-muted-foreground">
                          {result.description}
                        </span>
                      )}
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="flex items-center justify-between border-t px-3 py-2 text-xs text-muted-foreground">
            <div className="flex items-center gap-2">
              <kbd className="rounded border px-1.5 py-0.5">Enter</kbd>
              <span>pour selectionner</span>
            </div>
            <div className="flex items-center gap-2">
              <kbd className="rounded border px-1.5 py-0.5">Esc</kbd>
              <span>pour fermer</span>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
