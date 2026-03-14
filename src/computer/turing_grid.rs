use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub content: String,
    pub format: String,
    pub status: String,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuringGrid {
    pub cells: HashMap<String, Cell>,
    pub cursor: (i32, i32, i32),
    #[serde(skip)]
    pub persistence_path: PathBuf,
}

impl Default for TuringGrid {
    fn default() -> Self {
        Self::new(PathBuf::from("memory/turing_grid.json"))
    }
}

impl TuringGrid {
    pub fn new(persistence_path: PathBuf) -> Self {
        Self {
            cells: HashMap::new(),
            cursor: (0, 0, 0),
            persistence_path,
        }
    }

    fn coord_key(x: i32, y: i32, z: i32) -> String {
        format!("{},{},{}", x, y, z)
    }

    pub async fn load(persistence_path: PathBuf) -> std::io::Result<Self> {
        if persistence_path.exists() {
            let data = fs::read_to_string(&persistence_path).await?;
            if let Ok(mut grid) = serde_json::from_str::<TuringGrid>(&data) {
                grid.persistence_path = persistence_path;
                return Ok(grid);
            }
        }
        Ok(Self::new(persistence_path))
    }

    pub async fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.persistence_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_string_pretty(&self)?;
        fs::write(&self.persistence_path, data).await?;
        Ok(())
    }

    pub async fn move_cursor(&mut self, dx: i32, dy: i32, dz: i32) {
        self.cursor.0 += dx;
        self.cursor.1 += dy;
        self.cursor.2 += dz;
        
        // Clamp boundaries to prevent insane numbers
        self.cursor.0 = self.cursor.0.clamp(-2000, 2000);
        self.cursor.1 = self.cursor.1.clamp(-2000, 2000);
        self.cursor.2 = self.cursor.2.clamp(-2000, 2000);
        
        let _ = self.save().await; // Auto-persist on move
    }

    pub fn read_current(&self) -> Option<&Cell> {
        let key = Self::coord_key(self.cursor.0, self.cursor.1, self.cursor.2);
        self.cells.get(&key)
    }

    pub fn get_cursor(&self) -> (i32, i32, i32) {
        self.cursor
    }

    pub async fn write_current(&mut self, format: &str, content: &str) -> std::io::Result<()> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let cell = Cell {
            content: content.to_string(),
            format: format.to_string(),
            status: "Idle".to_string(),
            last_updated: timestamp,
        };
        let key = Self::coord_key(self.cursor.0, self.cursor.1, self.cursor.2);
        self.cells.insert(key, cell);
        self.save().await
    }
    
    pub async fn update_status(&mut self, status: &str) -> std::io::Result<()> {
        let key = Self::coord_key(self.cursor.0, self.cursor.1, self.cursor.2);
        if let Some(cell) = self.cells.get_mut(&key) {
            cell.status = status.to_string();
            cell.last_updated = chrono::Utc::now().to_rfc3339();
            return self.save().await;
        }
        Ok(())
    }

    pub fn scan(&self, radius: i32) -> Vec<((i32, i32, i32), String)> {
        let mut results = Vec::new();
        let (cx, cy, cz) = self.cursor;
        
        for (key, cell) in &self.cells {
            let parts: Vec<&str> = key.split(',').collect();
            if parts.len() == 3
                && let (Ok(x), Ok(y), Ok(z)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>(), parts[2].parse::<i32>())
                    && (x - cx).abs() <= radius && (y - cy).abs() <= radius && (z - cz).abs() <= radius {
                        results.push(((x, y, z), cell.format.clone()));
                    }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_grid_initialization() {
        let grid = TuringGrid::new(PathBuf::from("dummy.json"));
        assert_eq!(grid.cursor, (0, 0, 0));
        assert!(grid.cells.is_empty());
    }

    #[tokio::test]
    async fn test_grid_movement() {
        let mut grid = TuringGrid::new(PathBuf::from("dummy.json"));
        grid.move_cursor(5, -2, 10).await;
        assert_eq!(grid.get_cursor(), (5, -2, 10));
        
        // Test bounds clamping
        grid.move_cursor(3000, 0, 0).await;
        assert_eq!(grid.get_cursor(), (2000, -2, 10));
    }

    #[tokio::test]
    async fn test_grid_write_and_read() {
        let dir = env::temp_dir().join("hive_turing_test_rw");
        let path = dir.join("turing_grid.json");
        let mut grid = TuringGrid::new(path.clone());
        grid.move_cursor(1, 1, 1).await;
        
        grid.write_current("python", "print('hello 3D')").await.unwrap();
        
        let cell = grid.read_current().unwrap();
        assert_eq!(cell.format, "python");
        assert_eq!(cell.content, "print('hello 3D')");
        assert_eq!(cell.status, "Idle");
        
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn test_grid_scan() {
        let dir = env::temp_dir().join("hive_turing_test_scan");
        let mut grid = TuringGrid::new(dir.join("turing_grid.json"));
        
        grid.move_cursor(1, 0, 0).await;
        grid.write_current("text", "data 1").await.unwrap();
        
        grid.move_cursor(-2, 0, 0).await; // Cursor now at (-1, 0, 0)
        grid.write_current("sh", "echo 'bash'").await.unwrap();
        
        grid.move_cursor(1, 0, 0).await; // Back to origin (0, 0, 0)
        
        let scan = grid.scan(1);
        assert_eq!(scan.len(), 2);
        
        let scan_small = grid.scan(0);
        assert_eq!(scan_small.len(), 0);
        
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn test_grid_persistence() {
        let dir = env::temp_dir().join("hive_turing_test_persist");
        fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("turing_grid.json");
        
        let mut grid = TuringGrid::new(path.clone());
        grid.move_cursor(5, 5, 5).await;
        grid.write_current("text", "persistent data").await.unwrap();
        
        let reloaded_grid = TuringGrid::load(path.clone()).await.unwrap();
        assert_eq!(reloaded_grid.cursor, (5, 5, 5));
        
        let cell = reloaded_grid.read_current().unwrap();
        assert_eq!(cell.content, "persistent data");
        
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
    
    #[tokio::test]
    async fn test_update_status() {
        let dir = env::temp_dir().join("hive_turing_test_status");
        let path = dir.join("turing_grid.json");
        let mut grid = TuringGrid::new(path.clone());
        
        grid.write_current("python", "x = 1").await.unwrap();
        grid.update_status("Running").await.unwrap();
        
        let cell = grid.read_current().unwrap();
        assert_eq!(cell.status, "Running");
        
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}
